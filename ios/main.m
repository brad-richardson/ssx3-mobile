#import <Foundation/Foundation.h>
#import <UIKit/UIKit.h>
#import <AVFoundation/AVFoundation.h>
#import <GameController/GameController.h>
#include <stdio.h>
#include <stdlib.h>

extern void ssx3_ios_entry(bool (*scene_check)(void *));
extern void ssx3_ios_observe_window(void *ui_view);

bool ssx3_ios_scene_attached(void *ui_view) {
    UIWindowScene *target = nil;
    for (UIScene *scene in UIApplication.sharedApplication.connectedScenes) {
        if ([scene isKindOfClass:[UIWindowScene class]]) {
            target = (UIWindowScene *)scene;
            break;
        }
    }
    if (target == nil) return false;
    UIView *view = (__bridge UIView *)ui_view;
    UIWindow *window = view.window;
    if (window == nil) return false;
    if (window.windowScene == nil) window.windowScene = target;
    [window makeKeyAndVisible];
    ssx3_ios_observe_window(ui_view);
    return window.windowScene == target;
}

int main(int argc, char *argv[]) {
    @autoreleasepool {
        NSString *documents = NSSearchPathForDirectoriesInDomains(NSDocumentDirectory,
            NSUserDomainMask, YES).firstObject;
        if (documents == nil) return 2;
        setenv("SSX3_IOS_DOCUMENTS", documents.fileSystemRepresentation, 1);
        NSString *log = [documents stringByAppendingPathComponent:@"ssx3.log"];
        freopen(log.fileSystemRepresentation, "a", stderr);
        fprintf(stderr, "[ios] native_main documents=%s\n", documents.fileSystemRepresentation);
        fflush(stderr);
        ssx3_ios_entry(ssx3_ios_scene_attached);
    }
    return 0;
}
