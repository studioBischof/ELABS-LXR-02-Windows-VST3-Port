/* zigtool - makes Zig usable where a single `cc` / `dlltool` executable is expected.
 *
 * The sub-command comes from the executable's own name:
 *   zigcc.exe      ->  <ZIG> cc <args>
 *   dlltool.exe    ->  <ZIG> dlltool <args>
 *   ar.exe         ->  <ZIG> ar <args>
 *
 * For `cc`, a few arguments are adjusted for linking Rust's x86_64-pc-windows-gnu target.  rustc hands
 * long link lines over in an `@response-file` (one argument per line, backslashes doubled), so the
 * same rules are applied inside those files as well:
 *
 *   -nodefaultlibs      dropped. Zig's driver stops adding its built-in libc paths when it sees it and
 *                       then cannot find msvcrt/mingw32; Zig adds the same libs itself anyway.
 *   -lmsvcrt            dropped, same reason (Zig's libc is the CRT here).
 *   -l:libpthread.a     dropped: Rust's target spec names MinGW's winpthreads archive by file name;
 *                       Zig provides pthreads from its own libc.
 *   -lsynchronization   replaced by win/syslib/libsynchronization.a, a three-symbol import library for
 *                       the wait-on-address API (Zig ships no such import library).
 *   -Wl,<file>.def      dropped, with -Wl,--export-all-symbols added: rustc hands lld-link a .def as a
 *                       linker input (proc-macro export lists), a position Zig's driver rejects; a
 *                       proc-macro DLL has nothing to hide.
 *
 * ZIG must name zig.exe (set it in .cargo/config.toml).
 * Set ZIGTOOL_DEBUG=<file> to log every wrapper call's arguments.
 *
 * Build:  zig cc -target x86_64-windows-gnu -O2 -o win/zigcc.exe win/zigtool.c
 *         zig cc -target x86_64-windows-gnu -O2 -o win/dlltool.exe win/zigtool.c
 */
#include <ctype.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <process.h>
#include <windows.h>

static char g_dir[MAX_PATH];      /* directory of this executable */
static char g_syslib[MAX_PATH + 16];
static char g_ldir[32][MAX_PATH + 8];
static int g_nldir;
static int g_saw_def, g_saw_sync;
static char g_resp[MAX_PATH + 8]; /* the rewritten response file */

static void lower(char *s) { for (; *s; s++) *s = (char)tolower((unsigned char)*s); }

static void log_line(const char *s)
{
    const char *path = getenv("ZIGTOOL_DEBUG");
    FILE *f;
    if (!path || !*path) return;
    f = fopen(path, "a");
    if (!f) return;
    fprintf(f, "%s\n", s);
    fclose(f);
}

static char *read_file(const char *path)
{
    FILE *f = fopen(path, "rb");
    long n;
    char *buf;
    if (!f) return NULL;
    fseek(f, 0, SEEK_END);
    n = ftell(f);
    fseek(f, 0, SEEK_SET);
    buf = (char *)malloc((size_t)n + 1);
    if (!buf) { fclose(f); return NULL; }
    if (fread(buf, 1, (size_t)n, f) != (size_t)n) { free(buf); fclose(f); return NULL; }
    buf[n] = 0;
    fclose(f);
    return buf;
}

/* a response-file argument: "\\" -> '\', "\"" -> '"' */
static void unescape(char *s)
{
    char *w = s;
    while (*s) {
        if (s[0] == '\\' && (s[1] == '\\' || s[1] == '"')) s++;
        *w++ = *s++;
    }
    *w = 0;
}

static void write_arg(FILE *f, const char *s)
{
    for (; *s; s++) {
        if (*s == '\\' || *s == '"') fputc('\\', f);
        fputc(*s, f);
    }
    fputc('\n', f);
}

static int skip_cc(const char *a)
{
    return !strcmp(a, "-nodefaultlibs") || !strcmp(a, "-lmsvcrt") || !strcmp(a, "-l:libpthread.a");
}

/* Rust's target triples are not Zig's: x86_64-pc-windows-gnu -> x86_64-windows-gnu */
static const char *map_target(const char *t)
{
    if (!strcmp(t, "x86_64-pc-windows-gnu")) return "x86_64-windows-gnu";
    if (!strcmp(t, "aarch64-pc-windows-gnu")) return "aarch64-windows-gnu";
    if (!strcmp(t, "i686-pc-windows-gnu")) return "x86-windows-gnu";
    return t;
}

/* rustc's "-Wl,<path>.def" */
static int is_def_arg(const char *a)
{
    size_t n = strlen(a);
    return !strncmp(a, "-Wl,", 4) && n > 8 && !strcmp(a + n - 4, ".def");
}

static void add_ldir(const char *d)
{
    if (g_nldir >= 32 || !*d) return;
    snprintf(g_ldir[g_nldir], sizeof g_ldir[0], "%s", d);
    g_nldir++;
}

static int file_exists(const char *p)
{
    DWORD a = GetFileAttributesA(p);
    return a != INVALID_FILE_ATTRIBUTES && !(a & FILE_ATTRIBUTE_DIRECTORY);
}

/* Zig's driver treats "-l<name>" as a *dynamic* library and wants <name>.dll. MinGW's static
 * lib<name>.a in one of the -L directories is what the crates mean, so hand it over by path. */
static const char *resolve_lib(const char *a)
{
    static char out[MAX_PATH + 16];
    char file[MAX_PATH + 8];
    const char *name;
    int i;

    if (strncmp(a, "-l", 2) != 0 || !a[2]) return NULL;
    name = a + 2;
    if (name[0] == ':') snprintf(file, sizeof file, "%s", name + 1);
    else snprintf(file, sizeof file, "lib%s.a", name);
    for (i = 0; i < g_nldir; i++) {
        snprintf(out, sizeof out, "%s/%s", g_ldir[i], file);
        if (file_exists(out)) return out;
    }
    if (name[0] != ':') {
        snprintf(file, sizeof file, "lib%s.dll.a", name);
        for (i = 0; i < g_nldir; i++) {
            snprintf(out, sizeof out, "%s/%s", g_ldir[i], file);
            if (file_exists(out)) return out;
        }
    }
    return NULL;
}

/* filter one argument; returns a replacement (or NULL to keep it, or "" to drop it) */
static const char *filtered(const char *a)
{
    static char lib[MAX_PATH + 40];
    if (skip_cc(a)) return "";
    if (!strcmp(a, "-lsynchronization")) {
        if (!g_saw_sync) {
            g_saw_sync = 1;
            snprintf(lib, sizeof lib, "%s/libsynchronization.a", g_syslib);
            return lib;
        }
        return "";
    }
    if (is_def_arg(a)) { g_saw_def = 1; return ""; }
    return NULL;
}

/* rewrite a rustc response file with the same rules; returns 1 on success */
static int rewrite_response(const char *path)
{
    char *text = read_file(path);
    char *line, *next;
    FILE *out;
    int ok = 0;

    if (!text) return 0;
    snprintf(g_resp, sizeof g_resp, "%s.zt", path);
    out = fopen(g_resp, "wb");
    if (!out) { free(text); return 0; }

    for (line = text; line && *line; line = next) {
        bool drop = false;
        next = strchr(line, '\n');
        if (next) { *next++ = 0; } else { next = NULL; }
        if (*line && line[strlen(line) - 1] == '\r') line[strlen(line) - 1] = 0;
        if (!*line) continue;
        unescape(line);
        const char *rep = filtered(line);
        if (rep && !*rep) drop = true;
        if (!rep) { const char *r2 = resolve_lib(line); if (r2) rep = r2; }
        if (!drop) write_arg(out, rep ? rep : line);
        log_line(drop ? "  (dropped)" : (rep ? rep : line));
    }
    if (g_saw_def) write_arg(out, "-Wl,--export-all-symbols");
    fclose(out);
    free(text);
    ok = 1;
    return ok;
}

int main(int argc, char **argv)
{
    const char *zig = getenv("ZIG");
    char **args;
    char name[MAX_PATH];
    int i, n = 0, rc, is_cc;
    const char *sub;

    {   /* optional trace of every call (ZIGTOOL_DEBUG) */
        log_line("---- call ----");
        for (i = 0; i < argc; i++) log_line(argv[i]);
    }

    if (!zig || !*zig) {
        fprintf(stderr, "zigtool: set ZIG to the zig.exe path\n");
        return 1;
    }
    name[0] = 0;
    if (GetModuleFileNameA(NULL, name, (DWORD)sizeof name)) {
        char *slash = strrchr(name, '\\');
        char *base = slash ? slash + 1 : name;
        memmove(name, base, strlen(base) + 1);
    }
    lower(name);
    is_cc = strncmp(name, "zigcc", 5) == 0;
    sub = is_cc ? "cc" : strncmp(name, "dlltool", 7) == 0 ? "dlltool" : "ar";

    g_dir[0] = 0;
    {
        char exe[MAX_PATH];
        if (GetModuleFileNameA(NULL, exe, (DWORD)sizeof exe)) {
            char *slash = strrchr(exe, '\\');
            if (slash) *slash = 0;
            snprintf(g_dir, sizeof g_dir, "%s", exe);
        }
    }
    snprintf(g_syslib, sizeof g_syslib, "%s/syslib", g_dir);

    /* pass 1: the -L directories, from the command line and from any response files */
    if (is_cc) {
        for (i = 1; i < argc; i++) {
            if (!strncmp(argv[i], "-L", 2) && argv[i][2]) add_ldir(argv[i] + 2);
            else if (!strcmp(argv[i], "-L") && i + 1 < argc) add_ldir(argv[++i]);
            else if (argv[i][0] == '@') {
                char *text = read_file(argv[i] + 1);
                if (text) {
                    char *line = text, *next;
                    int want_path = 0;
                    while (line && *line) {
                        next = strchr(line, '\n');
                        if (next) *next++ = 0; else next = NULL;
                        if (*line) {
                            if (line[strlen(line) - 1] == '\r') line[strlen(line) - 1] = 0;
                            unescape(line);
                            if (want_path) { add_ldir(line); want_path = 0; }
                            else if (!strcmp(line, "-L")) want_path = 1;
                            else if (!strncmp(line, "-L", 2) && line[2]) add_ldir(line + 2);
                        }
                        line = next;
                    }
                    free(text);
                }
            }
        }
    }

    args = (char **)calloc((size_t)argc + 4, sizeof(char *));
    if (!args) return 1;
    args[n++] = (char *)zig;
    args[n++] = (char *)sub;
    for (i = 1; i < argc; i++) {
        if (is_cc && argv[i][0] == '@') {
            const char *rep = NULL;
            if (rewrite_response(argv[i] + 1)) {
                static char at[MAX_PATH + 12];
                snprintf(at, sizeof at, "@%s", g_resp);
                rep = at;
            }
            args[n++] = (char *)(rep ? rep : argv[i]);
            continue;
        }
        if (is_cc && !strncmp(argv[i], "--target=", 9)) {
            args[n++] = (char *)"-target";
            args[n++] = (char *)map_target(argv[i] + 9);
            continue;
        }
        if (is_cc && !strcmp(argv[i], "--target") && i + 1 < argc) {
            args[n++] = (char *)"-target";
            args[n++] = (char *)map_target(argv[++i]);
            continue;
        }
        if (is_cc) {
            const char *rep = filtered(argv[i]);
            if (rep && !*rep) continue;
            if (!rep) rep = resolve_lib(argv[i]);
            if (rep) { args[n++] = (char *)rep; continue; }
        }
        args[n++] = argv[i];
    }
    args[n] = NULL;

    rc = _spawnv(_P_WAIT, zig, (const char *const *)args);
    free(args);
    if (g_resp[0]) remove(g_resp);
    return rc < 0 ? 1 : rc;
}
