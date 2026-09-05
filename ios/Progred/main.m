#import <Foundation/Foundation.h>

extern void progred_start(void);

int main(int argc, char *argv[]) {
    @autoreleasepool {
        progred_start();
    }
    return 0;
}
