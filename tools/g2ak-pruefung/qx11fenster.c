/* G2-AK (QD-065): X11-Testfenster für die Vollbild-Prüfung unter Xwayland.
 *
 *   qx11fenster <modus> <sekunden> <id-datei>
 *
 * modus
 *   vorher    _NET_WM_STATE_FULLSCREEN steht vor dem Abbilden im Fenster
 *   danach    normal abbilden, nach 1 s Vollbild per _NET_WM_STATE-Nachricht
 *             an das Wurzelfenster (wie Programme mit eigenem Vollbild)
 *   gross     bildschirmgroß an 0,0 (USPosition), ohne Vollbild-Zustand
 *   verschoben normal abbilden, nach 1 s per ConfigureRequest bildschirmgroß
 *             an 0,0, ohne Vollbild-Zustand (wie ein Programm, das
 *             „Vollbild“ nur selbst zeichnet)
 *   override  bildschirmgroß an 0,0 mit override_redirect (unverwaltet)
 * Das Fenster ist rein rot (#ff0000); seine ID steht in <id-datei>. Läuft
 * nur, wenn DISPLAY gesetzt ist (in der isolierten labwc-Sitzung).
 */
#include <X11/Xatom.h>
#include <X11/Xlib.h>
#include <X11/Xutil.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

int main(int argc, char **argv)
{
    if (argc != 4) {
        fprintf(stderr, "Aufruf: qx11fenster vorher|danach|gross|verschoben|override <sekunden> <id-datei>\n");
        return 2;
    }
    const char *modus = argv[1];
    int sekunden = atoi(argv[2]);
    Display *d = XOpenDisplay(NULL);
    if (!d) { fprintf(stderr, "qx11fenster: keine X-Anzeige\n"); return 1; }
    int s = DefaultScreen(d);
    Window root = RootWindow(d, s);
    int w = DisplayWidth(d, s), h = DisplayHeight(d, s);
    int gross = !strcmp(modus, "gross") || !strcmp(modus, "override");

    XColor rot = { .red = 0xffff, .green = 0, .blue = 0, .flags = DoRed | DoGreen | DoBlue };
    XAllocColor(d, DefaultColormap(d, s), &rot);
    XSetWindowAttributes a = { .background_pixel = rot.pixel,
                               .override_redirect = !strcmp(modus, "override") };
    Window win = XCreateWindow(d, root, 0, 0, gross ? w : 400, gross ? h : 300, 0,
                               CopyFromParent, InputOutput, CopyFromParent,
                               CWBackPixel | CWOverrideRedirect, &a);
    XStoreName(d, win, "qx11fenster");
    XClassHint klasse = { .res_name = "qx11fenster", .res_class = "qx11fenster" };
    XSetClassHint(d, win, &klasse);
    if (gross) {
        XSizeHints *hint = XAllocSizeHints();
        hint->flags = USPosition | USSize;
        hint->x = 0; hint->y = 0; hint->width = w; hint->height = h;
        XSetWMNormalHints(d, win, hint);
        XFree(hint);
    }
    Atom state = XInternAtom(d, "_NET_WM_STATE", False);
    Atom voll = XInternAtom(d, "_NET_WM_STATE_FULLSCREEN", False);
    if (!strcmp(modus, "vorher"))
        XChangeProperty(d, win, state, XA_ATOM, 32, PropModeReplace, (unsigned char *)&voll, 1);
    XMapRaised(d, win);
    XFlush(d);

    FILE *f = fopen(argv[3], "w");
    if (f) { fprintf(f, "0x%lx\n", win); fclose(f); }
    printf("qx11fenster %s: Fenster 0x%lx, Bildschirm %dx%d\n", modus, win, w, h);
    fflush(stdout);

    for (int ms = 0; ms < sekunden * 1000; ms += 50) {
        while (XPending(d)) { XEvent e; XNextEvent(d, &e); }
        if (ms == 1000 && !strcmp(modus, "danach")) {
            XEvent e = { 0 };
            e.xclient.type = ClientMessage;
            e.xclient.window = win;
            e.xclient.message_type = state;
            e.xclient.format = 32;
            e.xclient.data.l[0] = 1; /* _NET_WM_STATE_ADD */
            e.xclient.data.l[1] = (long)voll;
            e.xclient.data.l[3] = 1; /* Quelle: normales Programm */
            XSendEvent(d, root, False, SubstructureRedirectMask | SubstructureNotifyMask, &e);
            XFlush(d);
            printf("qx11fenster: Vollbild angefragt\n");
            fflush(stdout);
        }
        if (ms == 1000 && !strcmp(modus, "verschoben")) {
            XMoveResizeWindow(d, win, 0, 0, w, h);
            XFlush(d);
            printf("qx11fenster: %dx%d an 0,0 angefragt\n", w, h);
            fflush(stdout);
        }
        usleep(50000);
    }
    XCloseDisplay(d);
    return 0;
}
