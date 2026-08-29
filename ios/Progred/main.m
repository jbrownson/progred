#import <Foundation/Foundation.h>
#include "rust-build.h"

extern void progred_start(void);

int main(int argc, char *argv[]) {
    @autoreleasepool {
        progred_start();
    }
    return 0;
}
