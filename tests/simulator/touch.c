/* Simulator-only SDL touch injection. No hook is compiled into production.
 * Fixed-size FIFO messages allow the real event loop to receive finger events.
 */
#define _GNU_SOURCE
#include <SDL2/SDL.h>
#include <dlfcn.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>

static int touch(SDL_Event *event) {
    static int input = -1;
    const char *path = getenv("VITRALLIS_TEST_TOUCH_FIFO");
    if (!path) return 0;
    if (input < 0) input = open(path, O_RDONLY | O_NONBLOCK | O_CLOEXEC);
    if (input < 0) return 0;
    char message[65] = {0}, kind;
    float x, y;
    if (read(input, message, 64) != 64 || sscanf(message, "%c %f %f", &kind, &x, &y) != 3) return 0;
    SDL_zero(*event);
    event->type = kind == 'D' ? SDL_FINGERDOWN : SDL_FINGERUP;
    event->tfinger.timestamp = SDL_GetTicks();
    event->tfinger.touchId = 1;
    event->tfinger.fingerId = 1;
    event->tfinger.x = x;
    event->tfinger.y = y;
    event->tfinger.pressure = kind == 'D' ? 1 : 0;
    return 1;
}
int SDL_PollEvent(SDL_Event *event) {
    static int (*real)(SDL_Event *);
    if (!real) real = dlsym(RTLD_NEXT, "SDL_PollEvent");
    return real(event) || (event && touch(event));
}
int SDL_WaitEventTimeout(SDL_Event *event, int timeout) {
    static int (*real)(SDL_Event *, int);
    if (!real) real = dlsym(RTLD_NEXT, "SDL_WaitEventTimeout");
    if (event && touch(event)) return 1;
    return real(event, timeout < 0 || timeout > 20 ? 20 : timeout);
}
