// Link with the same pinned Piper and eSpeak archives as the app.
#include <stdio.h>
#include <string.h>
#include "c-api.h"
#include "espeak-ng/speak_lib.h"

int main(int argc, char **argv) {
    if (argc != 2 || strlen(argv[1]) < 261) {
        fprintf(stderr, "Supply an espeak-ng-data path of at least 261 bytes\n");
        return 2;
    }
    if (PiperPhonemizeInitialize(argv[1]) <= 0) {
        fprintf(stderr, "Native initialization failed\n");
        return 1;
    }
    const char *loaded = NULL;
    espeak_Info(&loaded);
    if (!loaded || strcmp(loaded, argv[1]) != 0) {
        fprintf(stderr, "Native library selected a different data directory\n");
        return 1;
    }
    PiperPhonemizeResult *result = PiperPhonemizeText("East Pier at Dawn", "en-us");
    int count = result && PiperPhonemizeResultGetNumSentences(result) > 0
        ? PiperPhonemizeResultGetNumPhonemes(result, 0) : 0;
    if (result) PiperPhonemizeDestroyResult(result);
    printf("%zu-byte path, %d phonemes\n", strlen(loaded), count);
    return count > 0 ? 0 : 1;
}
