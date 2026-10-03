/* Signed bundle identity probe for macOS updater component tests only. */
#include <stdio.h>
#include <string.h>

#ifndef INSTPLOT_PROBE_VERSION
#error INSTPLOT_PROBE_VERSION is required
#endif

int main(int argc, char **argv) {
    if (argc == 2 && strcmp(argv[1], "--product-info") == 0) {
        puts("InstPlot Studio\tinstplot-studio\t" INSTPLOT_PROBE_VERSION);
        return 0;
    }
    if (argc == 2 && strcmp(argv[1], "--update-protocol") == 0) {
        puts("1");
        return 0;
    }
    return 7; /* A probe never launches a GUI or fakes a healthy startup. */
}
