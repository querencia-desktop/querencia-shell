// qeingabe: input for the ISOLATED GUI test only (HMP-05).
//
// Creates a virtual pointer (zwlr_virtual_pointer_v1) and a virtual keyboard
// (zwp_virtual_keyboard_v1) on the compositor named by WAYLAND_DISPLAY and
// replays commands from stdin. Refuses to run unless XDG_RUNTIME_DIR is a
// throw-away directory of isoliert.py (".../hmp05-runtime-*"), so it can
// never inject input into the running desktop session.
//
// Commands (one per line; coordinates in the 1280x720 headless output):
//   move X Y          jump the pointer
//   glide X Y MS      move in 10 ms steps over MS milliseconds
//   click             left button press + release
//   rclick / dclick   right click / left double click
//   down B / up B     press / release button B (left, middle, right) alone,
//                     for drags with glide in between (Teil 5)
//   run CMD           start CMD in the background (test windows, HMP-06)
//   scroll N          N wheel notches, positive = down (HMP-07)
//   key NAME          Escape Down Up Left Right Return
//   wait MS
//   shot FILE         grim FILE
//   mark TEXT         print TEXT with a timestamp (for the logs)
#define _GNU_SOURCE
#include <linux/input-event-codes.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <time.h>
#include <unistd.h>
#include <wayland-client.h>
#include <xkbcommon/xkbcommon.h>
#include "virtual-keyboard-unstable-v1-client-protocol.h"
#include "wlr-virtual-pointer-unstable-v1-client-protocol.h"

static struct wl_seat *seat;
static struct zwlr_virtual_pointer_manager_v1 *pointer_manager;
static struct zwp_virtual_keyboard_manager_v1 *keyboard_manager;
/* G2-J: Ausdehnung des Zeigerbereichs = Auflösung der Testausgabe, aus
 * QE_W/QE_H (Standard 1280x720, wie W2). */
static unsigned W = 1280, H = 720;
static double px = 640, py = 600;

static void global(void *data, struct wl_registry *registry, uint32_t name,
                   const char *interface, uint32_t version) {
    (void)data; (void)version;
    if (!strcmp(interface, wl_seat_interface.name))
        seat = wl_registry_bind(registry, name, &wl_seat_interface, 1);
    else if (!strcmp(interface, zwlr_virtual_pointer_manager_v1_interface.name))
        pointer_manager = wl_registry_bind(registry, name,
            &zwlr_virtual_pointer_manager_v1_interface, 1);
    else if (!strcmp(interface, zwp_virtual_keyboard_manager_v1_interface.name))
        keyboard_manager = wl_registry_bind(registry, name,
            &zwp_virtual_keyboard_manager_v1_interface, 1);
}
static void global_remove(void *d, struct wl_registry *r, uint32_t n) { (void)d; (void)r; (void)n; }
static const struct wl_registry_listener listener = { global, global_remove };

static uint32_t now_ms(void) {
    struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t);
    return (uint32_t)(t.tv_sec * 1000 + t.tv_nsec / 1000000);
}
static void sleep_ms(long ms) {
    struct timespec t = { ms / 1000, (ms % 1000) * 1000000L }; nanosleep(&t, NULL);
}


// German (de) layout: evdev key and Shift for the characters "type" sends.
static int de_key(char c, int *shift) {
    static const char *row1 = "qwertzuiop", *row2 = "asdfghjkl", *row3 = "yxcvbnm";
    static const int k1[] = {KEY_Q, KEY_W, KEY_E, KEY_R, KEY_T, KEY_Y, KEY_U, KEY_I, KEY_O, KEY_P};
    static const int k2[] = {KEY_A, KEY_S, KEY_D, KEY_F, KEY_G, KEY_H, KEY_J, KEY_K, KEY_L};
    static const int k3[] = {KEY_Z, KEY_X, KEY_C, KEY_V, KEY_B, KEY_N, KEY_M};
    static const int digits[] = {KEY_0, KEY_1, KEY_2, KEY_3, KEY_4, KEY_5, KEY_6, KEY_7, KEY_8, KEY_9};
    *shift = 0;
    if (c >= 'A' && c <= 'Z') { *shift = 1; c = (char)(c - 'A' + 'a'); }
    const char *p;
    if ((p = strchr(row1, c)) && c) return k1[p - row1];
    if ((p = strchr(row2, c)) && c) return k2[p - row2];
    if ((p = strchr(row3, c)) && c) return k3[p - row3];
    if (c >= '0' && c <= '9') return digits[c - '0'];
    switch (c) {
    case '/': *shift = 1; return KEY_7;
    case '-': return KEY_SLASH;
    case '_': *shift = 1; return KEY_SLASH;
    case '.': return KEY_DOT;
    case ' ': return KEY_SPACE;
    default: return -1;
    }
}

int main(void) {
    const char *runtime = getenv("XDG_RUNTIME_DIR");
    if (!runtime || strncmp(runtime, "/run/user/", 10) == 0
        || !strstr(runtime, "/querencia-runtime-wt-netzwerk/nachweis/gui/rt/g2m-")) {
        fprintf(stderr, "qeingabe: Abbruch, nur in der isolierten Sitzung (XDG_RUNTIME_DIR=%s)\n",
                runtime ? runtime : "");
        return 2;
    }
    if (getenv("QE_W") && getenv("QE_H")) { W = (unsigned)atoi(getenv("QE_W")); H = (unsigned)atoi(getenv("QE_H")); }
    if (W < 320 || H < 240) { fprintf(stderr, "qeingabe: QE_W/QE_H ungültig\n"); return 2; }
    struct wl_display *display = wl_display_connect(NULL);
    if (!display) { fprintf(stderr, "qeingabe: kein Wayland\n"); return 2; }
    struct wl_registry *registry = wl_display_get_registry(display);
    wl_registry_add_listener(registry, &listener, NULL);
    wl_display_roundtrip(display);
    if (!seat || !pointer_manager || !keyboard_manager) {
        fprintf(stderr, "qeingabe: Protokolle fehlen\n"); return 2;
    }
    struct zwlr_virtual_pointer_v1 *pointer =
        zwlr_virtual_pointer_manager_v1_create_virtual_pointer(pointer_manager, seat);
    struct zwp_virtual_keyboard_v1 *keyboard =
        zwp_virtual_keyboard_manager_v1_create_virtual_keyboard(keyboard_manager, seat);

    struct xkb_context *ctx = xkb_context_new(XKB_CONTEXT_NO_FLAGS);
    struct xkb_rule_names names = { .layout = "de" };
    struct xkb_keymap *keymap = xkb_keymap_new_from_names(ctx, &names, XKB_KEYMAP_COMPILE_NO_FLAGS);
    char *text = xkb_keymap_get_as_string(keymap, XKB_KEYMAP_FORMAT_TEXT_V1);
    size_t size = strlen(text) + 1;
    int fd = memfd_create("keymap", MFD_CLOEXEC);
    if (fd < 0 || write(fd, text, size) != (ssize_t)size) { perror("keymap"); return 2; }
    zwp_virtual_keyboard_v1_keymap(keyboard, WL_KEYBOARD_KEYMAP_FORMAT_XKB_V1, fd, size);
    wl_display_roundtrip(display);

    char line[512];
    while (fgets(line, sizeof line, stdin)) {
        char cmd[32] = "", arg[400] = "";
        double x = 0, y = 0; long ms = 0;
        if (sscanf(line, "%31s", cmd) != 1 || cmd[0] == '#') continue;
        if (!strcmp(cmd, "move") && sscanf(line, "%*s %lf %lf", &x, &y) == 2) {
            px = x; py = y;
            zwlr_virtual_pointer_v1_motion_absolute(pointer, now_ms(), (uint32_t)x, (uint32_t)y, W, H);
            zwlr_virtual_pointer_v1_frame(pointer);
        } else if (!strcmp(cmd, "glide") && sscanf(line, "%*s %lf %lf %ld", &x, &y, &ms) == 3) {
            long steps = ms / 10 > 0 ? ms / 10 : 1;
            double sx = px, sy = py;
            for (long i = 1; i <= steps; i++) {
                px = sx + (x - sx) * i / steps; py = sy + (y - sy) * i / steps;
                zwlr_virtual_pointer_v1_motion_absolute(pointer, now_ms(), (uint32_t)px, (uint32_t)py, W, H);
                zwlr_virtual_pointer_v1_frame(pointer);
                wl_display_flush(display);
                sleep_ms(10);
            }
        } else if (!strcmp(cmd, "click") || !strcmp(cmd, "rclick") || !strcmp(cmd, "dclick") || !strcmp(cmd, "mclick")) {
            uint32_t button = !strcmp(cmd, "rclick") ? BTN_RIGHT : !strcmp(cmd, "mclick") ? BTN_MIDDLE : BTN_LEFT;
            for (int n = !strcmp(cmd, "dclick") ? 2 : 1; n > 0; n--) {
                zwlr_virtual_pointer_v1_button(pointer, now_ms(), button, WL_POINTER_BUTTON_STATE_PRESSED);
                zwlr_virtual_pointer_v1_frame(pointer);
                wl_display_roundtrip(display); sleep_ms(40);
                zwlr_virtual_pointer_v1_button(pointer, now_ms(), button, WL_POINTER_BUTTON_STATE_RELEASED);
                zwlr_virtual_pointer_v1_frame(pointer);
                wl_display_roundtrip(display);
                if (n > 1) sleep_ms(80);
            }
        } else if ((!strcmp(cmd, "down") || !strcmp(cmd, "up")) && sscanf(line, "%*s %399s", arg) == 1) {
            uint32_t button = !strcmp(arg, "middle") ? BTN_MIDDLE : !strcmp(arg, "right") ? BTN_RIGHT : BTN_LEFT;
            zwlr_virtual_pointer_v1_button(pointer, now_ms(), button,
                !strcmp(cmd, "down") ? WL_POINTER_BUTTON_STATE_PRESSED : WL_POINTER_BUTTON_STATE_RELEASED);
            zwlr_virtual_pointer_v1_frame(pointer);
            wl_display_roundtrip(display);
        } else if (!strcmp(cmd, "scroll") && sscanf(line, "%*s %ld", &ms) == 1) {
            // ms = notches: positive = down (next), negative = up
            zwlr_virtual_pointer_v1_axis_source(pointer, WL_POINTER_AXIS_SOURCE_WHEEL);
            zwlr_virtual_pointer_v1_axis_discrete(pointer, now_ms(), WL_POINTER_AXIS_VERTICAL_SCROLL,
                                                  wl_fixed_from_int(15 * (int)ms), (int32_t)ms);
            zwlr_virtual_pointer_v1_frame(pointer);
        } else if (!strcmp(cmd, "run")) {
            // start a test program in the background, in this (isolated) session
            char sh[600]; snprintf(sh, sizeof sh, "(%.*s) >/dev/null 2>&1 &", (int)strcspn(line + 4, "\n"), line + 4);
            if (system(sh) != 0) fprintf(stderr, "qeingabe: run fehlgeschlagen\n");
        } else if ((!strcmp(cmd, "keydown") /* chords: keydown Alt_L, keydown F4, keyup … */ || !strcmp(cmd, "keyup")) && sscanf(line, "%*s %399s", arg) == 1) {
            int code = !strcmp(arg, "Control_L") ? KEY_LEFTCTRL : !strcmp(arg, "Alt_L") ? KEY_LEFTALT
                     : !strcmp(arg, "Shift_L") ? KEY_LEFTSHIFT : !strcmp(arg, "Super_L") ? KEY_LEFTMETA
                     : !strcmp(arg, "F4") ? KEY_F4
                     : !strcmp(arg, "Left") ? KEY_LEFT
                     : !strcmp(arg, "Right") ? KEY_RIGHT : -1;
            if (code < 0) { fprintf(stderr, "qeingabe: Taste %s?\n", arg); continue; }
            zwp_virtual_keyboard_v1_key(keyboard, now_ms(), code, !strcmp(cmd, "keydown")
                                        ? WL_KEYBOARD_KEY_STATE_PRESSED : WL_KEYBOARD_KEY_STATE_RELEASED);
            // wlroots does not derive modifiers from virtual key events
            // (update_state = false): send the modifier state explicitly.
            static uint32_t mods;
            uint32_t bit = code == KEY_LEFTCTRL ? 4 : code == KEY_LEFTALT ? 8 : code == KEY_LEFTSHIFT ? 1
                         : code == KEY_LEFTMETA ? 64 : 0;
            if (bit) {
                mods = !strcmp(cmd, "keydown") ? (mods | bit) : (mods & ~bit);
                zwp_virtual_keyboard_v1_modifiers(keyboard, mods, 0, 0, 0);
            }
            wl_display_roundtrip(display); sleep_ms(30);
        } else if (!strcmp(cmd, "key") && sscanf(line, "%*s %399s", arg) == 1) {
            int code = !strcmp(arg, "Escape") ? KEY_ESC : !strcmp(arg, "Down") ? KEY_DOWN
                     : !strcmp(arg, "Up") ? KEY_UP : !strcmp(arg, "Left") ? KEY_LEFT
                     : !strcmp(arg, "Right") ? KEY_RIGHT : !strcmp(arg, "Return") ? KEY_ENTER
                     : !strcmp(arg, "Tab") ? KEY_TAB : !strcmp(arg, "F2") ? KEY_F2
                     : !strcmp(arg, "Delete") ? KEY_DELETE : !strcmp(arg, "Print") ? KEY_SYSRQ
                     : !strcmp(arg, "Page_Up") ? KEY_PAGEUP : !strcmp(arg, "Page_Down") ? KEY_PAGEDOWN
                     : !strcmp(arg, "Home") ? KEY_HOME : !strcmp(arg, "End") ? KEY_END
                     : !strcmp(arg, "space") ? KEY_SPACE : -1;
            if (code < 0) { fprintf(stderr, "qeingabe: Taste %s?\n", arg); continue; }
            zwp_virtual_keyboard_v1_key(keyboard, now_ms(), code, WL_KEYBOARD_KEY_STATE_PRESSED);
            wl_display_roundtrip(display); sleep_ms(30);
            zwp_virtual_keyboard_v1_key(keyboard, now_ms(), code, WL_KEYBOARD_KEY_STATE_RELEASED);
        } else if (!strcmp(cmd, "type")) {
            // type TEXT: letters, digits, space and / - _ . on the de layout
            const char *t = line + 5;
            for (; *t && *t != '\n'; t++) {
                int shift, code = de_key(*t, &shift);
                if (code < 0) { fprintf(stderr, "qeingabe: Zeichen %c?\n", *t); continue; }
                if (shift) {
                    zwp_virtual_keyboard_v1_key(keyboard, now_ms(), KEY_LEFTSHIFT, WL_KEYBOARD_KEY_STATE_PRESSED);
                    zwp_virtual_keyboard_v1_modifiers(keyboard, 1, 0, 0, 0);
                }
                zwp_virtual_keyboard_v1_key(keyboard, now_ms(), code, WL_KEYBOARD_KEY_STATE_PRESSED);
                wl_display_roundtrip(display); sleep_ms(15);
                zwp_virtual_keyboard_v1_key(keyboard, now_ms(), code, WL_KEYBOARD_KEY_STATE_RELEASED);
                if (shift) {
                    zwp_virtual_keyboard_v1_key(keyboard, now_ms(), KEY_LEFTSHIFT, WL_KEYBOARD_KEY_STATE_RELEASED);
                    zwp_virtual_keyboard_v1_modifiers(keyboard, 0, 0, 0, 0);
                }
                wl_display_roundtrip(display); sleep_ms(15);
            }
        } else if (!strcmp(cmd, "wait") && sscanf(line, "%*s %ld", &ms) == 1) {
            wl_display_flush(display); sleep_ms(ms);
        } else if (!strcmp(cmd, "shot") && sscanf(line, "%*s %399s", arg) == 1) {
            wl_display_roundtrip(display); sleep_ms(150);
            char sh[512]; snprintf(sh, sizeof sh, "grim '%s'", arg);
            if (system(sh) != 0) fprintf(stderr, "qeingabe: grim fehlgeschlagen\n");
        } else if (!strcmp(cmd, "mark")) {
            struct timespec t; clock_gettime(CLOCK_REALTIME, &t);
            struct tm tm; localtime_r(&t.tv_sec, &tm);
            printf("MARKE %02d:%02d:%02d.%03ld %s", tm.tm_hour, tm.tm_min, tm.tm_sec,
                   t.tv_nsec / 1000000, line + 5);
            fflush(stdout);
        } else {
            fprintf(stderr, "qeingabe: unbekannt: %s", line);
        }
        wl_display_roundtrip(display);
    }
    zwp_virtual_keyboard_v1_destroy(keyboard);
    zwlr_virtual_pointer_v1_destroy(pointer);
    wl_display_roundtrip(display);
    wl_display_disconnect(display);
    return 0;
}
