#import <Foundation/Foundation.h>
#import <AVFoundation/AVFoundation.h>
#import <GameController/GameController.h>
#import <UIKit/UIKit.h>
#include <stdint.h>
#include <stdio.h>

extern void ssx3_ios_audio_interrupted(bool interrupted);
static id interruptionObserver;
static id routeObserver;
static id appActiveObserver;
static id appInactiveObserver;
static id sceneActiveObserver;
static id sceneInactiveObserver;
static id controllerConnectObserver;
static id controllerDisconnectObserver;
static id controllerCurrentObserver;
static id controllerOldCurrentObserver;
static UIWindow *gameWindow;

static const char *ssx3_ios_audio_name(NSString *value) {
    return value ? value.UTF8String : "(nil)";
}

bool ssx3_ios_audio_activate(int mode) {
    AVAudioSession *session = AVAudioSession.sharedInstance;
    if (interruptionObserver == nil) {
        interruptionObserver = [NSNotificationCenter.defaultCenter
            addObserverForName:AVAudioSessionInterruptionNotification object:session
            queue:NSOperationQueue.mainQueue usingBlock:^(NSNotification *note) {
                NSNumber *kind = note.userInfo[AVAudioSessionInterruptionTypeKey];
                bool began = kind.unsignedIntegerValue == AVAudioSessionInterruptionTypeBegan;
                fprintf(stderr, "[ios-audio] AVAudioSessionInterruptionNotification=%s\n", began ? "began" : "ended");
                fflush(stderr);
                ssx3_ios_audio_interrupted(began);
            }];
    }
    NSString *wanted = mode == 1 ? AVAudioSessionCategoryAmbient : AVAudioSessionCategoryPlayback;
    NSError *error = nil;
    AVAudioSessionCategoryOptions options = mode == 1 ? 0 : AVAudioSessionCategoryOptionMixWithOthers;
    if (![session setCategory:wanted withOptions:options error:&error]) {
        fprintf(stderr, "[ios-audio] mode=%s stage=setCategory failed=%s actual=%s\n",
            mode == 1 ? "ambient" : "playback", ssx3_ios_audio_name(error.localizedDescription),
            ssx3_ios_audio_name(session.category));
        fflush(stderr);
        return false;
    }
    // These are preferences; an unsupported request must not skip activation.
    NSError *preferenceError = nil;
    if (![session setPreferredSampleRate:48000 error:&preferenceError]) {
        fprintf(stderr, "[ios-audio] preferred_rate_error=%s\n",
            ssx3_ios_audio_name(preferenceError.localizedDescription));
    }
    preferenceError = nil;
    if (![session setPreferredIOBufferDuration:(512.0 / 48000.0) error:&preferenceError]) {
        fprintf(stderr, "[ios-audio] preferred_buffer_error=%s\n",
            ssx3_ios_audio_name(preferenceError.localizedDescription));
    }
    error = nil;
    if (![session setActive:YES error:&error]) {
        fprintf(stderr, "[ios-audio] mode=%s stage=setActive failed=%s actual=%s\n",
            mode == 1 ? "ambient" : "playback", ssx3_ios_audio_name(error.localizedDescription),
            ssx3_ios_audio_name(session.category));
        fflush(stderr);
        return false;
    }
    bool matches = [session.category isEqualToString:wanted] &&
        (mode == 1 || (session.categoryOptions & AVAudioSessionCategoryOptionMixWithOthers));
    fprintf(stderr, "[ios-audio] mode=%s session_activated=true category=%s options=%lu matches=%s rate=%.0f buffer_ms=%.3f\n",
        mode == 1 ? "ambient" : "playback", ssx3_ios_audio_name(session.category),
        (unsigned long)session.categoryOptions, matches ? "true" : "false",
        session.sampleRate, session.IOBufferDuration * 1000);
    fflush(stderr);
    return matches;
}

// CoreAudio stream setup may change the process-wide session. Only a normal
// Playback launch calls this; explicit ambient/off test modes keep their policy.
bool ssx3_ios_audio_confirm_playback(void) {
    AVAudioSession *session = AVAudioSession.sharedInstance;
    NSError *error = nil;
    if (![session setCategory:AVAudioSessionCategoryPlayback
            withOptions:AVAudioSessionCategoryOptionMixWithOthers error:&error] ||
        ![session setActive:YES error:&error]) {
        fprintf(stderr, "[ios-audio] stream_playback_reassert_failed=%s actual=%s options=%lu\n",
            ssx3_ios_audio_name(error.localizedDescription), ssx3_ios_audio_name(session.category),
            (unsigned long)session.categoryOptions);
        fflush(stderr);
        return false;
    }
    bool playback = [session.category isEqualToString:AVAudioSessionCategoryPlayback] &&
        (session.categoryOptions & AVAudioSessionCategoryOptionMixWithOthers);
    fprintf(stderr, "[ios-audio] stream_category=%s options=%lu playback_mixing=%s\n",
        ssx3_ios_audio_name(session.category), (unsigned long)session.categoryOptions,
        playback ? "true" : "false");
    fflush(stderr);
    return playback;
}

void ssx3_ios_audio_deactivate(void) {
    NSError *error = nil;
    if (![AVAudioSession.sharedInstance setActive:NO error:&error])
        fprintf(stderr, "[ios-audio] deactivate failed: %s\n", error.localizedDescription.UTF8String);
}

static void ssx3_ios_log_focus(const char *event, bool restoredKey) {
    UIApplication *app = UIApplication.sharedApplication;
    UIScene *scene = gameWindow.windowScene;
    fprintf(stderr, "[ios-focus] event=%s app_state=%ld scene_state=%ld key=%d restored_key=%d controllers=%lu current=%p\n",
        event, (long)app.applicationState, scene ? (long)scene.activationState : -1L,
        gameWindow.isKeyWindow, restoredKey, (unsigned long)GCController.controllers.count,
        GCController.current);
    fflush(stderr);
}

static void ssx3_ios_restore_key_window(const char *event) {
    UIApplication *app = UIApplication.sharedApplication;
    bool restored = false;
    if (gameWindow && !gameWindow.isKeyWindow &&
        app.applicationState == UIApplicationStateActive &&
        gameWindow.windowScene.activationState == UISceneActivationStateForegroundActive) {
        [gameWindow makeKeyAndVisible];
        restored = true;
    }
    ssx3_ios_log_focus(event, restored);
}

bool ssx3_ios_app_active(void) {
    return gameWindow && UIApplication.sharedApplication.applicationState == UIApplicationStateActive &&
        gameWindow.windowScene.activationState == UISceneActivationStateForegroundActive;
}

// Keep observers for the process lifetime. A route sheet may temporarily take
// focus without a matching winit resume event; log each transition and restore
// the existing game window when the scene is active again.
void ssx3_ios_observe_window(void *ui_view) {
    UIView *view = (__bridge UIView *)ui_view;
    gameWindow = view.window;
    if (routeObserver != nil) {
        ssx3_ios_restore_key_window("window_attached_again");
        return;
    }
    NSNotificationCenter *center = NSNotificationCenter.defaultCenter;
    routeObserver = [center addObserverForName:AVAudioSessionRouteChangeNotification object:nil
        queue:NSOperationQueue.mainQueue usingBlock:^(NSNotification *note) {
            NSNumber *reason = note.userInfo[AVAudioSessionRouteChangeReasonKey];
            AVAudioSession *session = AVAudioSession.sharedInstance;
            AVAudioSessionPortDescription *output = session.currentRoute.outputs.firstObject;
            fprintf(stderr, "[ios-audio] route_change reason=%lu category=%s options=%lu output_port=%s\n",
                (unsigned long)reason.unsignedIntegerValue, ssx3_ios_audio_name(session.category),
                (unsigned long)session.categoryOptions, ssx3_ios_audio_name(output.portType));
            fflush(stderr);
            ssx3_ios_restore_key_window("route_change");
        }];
    appActiveObserver = [center addObserverForName:UIApplicationDidBecomeActiveNotification object:nil
        queue:NSOperationQueue.mainQueue usingBlock:^(NSNotification *note) {
            ssx3_ios_restore_key_window("app_did_become_active");
        }];
    appInactiveObserver = [center addObserverForName:UIApplicationWillResignActiveNotification object:nil
        queue:NSOperationQueue.mainQueue usingBlock:^(NSNotification *note) {
            ssx3_ios_log_focus("app_will_resign_active", false);
        }];
    sceneActiveObserver = [center addObserverForName:UISceneDidActivateNotification object:nil
        queue:NSOperationQueue.mainQueue usingBlock:^(NSNotification *note) {
            if (note.object == gameWindow.windowScene) ssx3_ios_restore_key_window("scene_did_activate");
        }];
    sceneInactiveObserver = [center addObserverForName:UISceneWillDeactivateNotification object:nil
        queue:NSOperationQueue.mainQueue usingBlock:^(NSNotification *note) {
            if (note.object == gameWindow.windowScene) ssx3_ios_log_focus("scene_will_deactivate", false);
        }];
    controllerConnectObserver = [center addObserverForName:GCControllerDidConnectNotification object:nil
        queue:NSOperationQueue.mainQueue usingBlock:^(NSNotification *note) {
            ssx3_ios_log_focus("controller_connected", false);
        }];
    controllerDisconnectObserver = [center addObserverForName:GCControllerDidDisconnectNotification object:nil
        queue:NSOperationQueue.mainQueue usingBlock:^(NSNotification *note) {
            ssx3_ios_log_focus("controller_disconnected", false);
        }];
    controllerCurrentObserver = [center addObserverForName:GCControllerDidBecomeCurrentNotification object:nil
        queue:NSOperationQueue.mainQueue usingBlock:^(NSNotification *note) {
            ssx3_ios_log_focus("controller_became_current", false);
        }];
    controllerOldCurrentObserver = [center addObserverForName:GCControllerDidStopBeingCurrentNotification object:nil
        queue:NSOperationQueue.mainQueue usingBlock:^(NSNotification *note) {
            ssx3_ios_log_focus("controller_stopped_being_current", false);
        }];
    ssx3_ios_restore_key_window("window_attached");
}

// Read the first extended controller on the main thread. A freshly connected
// controller becomes usable without binding a callback or owning game state.
uint32_t ssx3_ios_gc_poll(float *sticks, bool *has_select) {
    sticks[0] = sticks[1] = sticks[2] = sticks[3] = 0;
    *has_select = false;
    GCController *selected = GCController.current;
    if (selected.extendedGamepad == nil) {
        selected = nil;
        for (GCController *controller in GCController.controllers) {
            if (controller.extendedGamepad != nil) { selected = controller; break; }
        }
    }
    static GCController *lastSelected;
    if (selected != lastSelected) {
        fprintf(stderr, "[ios-input] selected_controller=%p current=%p count=%lu\n",
            selected, GCController.current, (unsigned long)GCController.controllers.count);
        fflush(stderr);
        lastSelected = selected;
    }
    GCExtendedGamepad *g = selected.extendedGamepad;
    if (g == nil) return 0;
    uint32_t b = 0;
    if (g.buttonA.pressed) b |= 1u<<0;
    if (g.buttonB.pressed) b |= 1u<<1;
    if (g.buttonX.pressed) b |= 1u<<2;
    if (g.buttonY.pressed) b |= 1u<<3;
    if (g.buttonMenu.pressed) b |= 1u<<4;
    if (g.buttonOptions.pressed) b |= 1u<<5;
    if (g.leftShoulder.pressed) b |= 1u<<6;
    if (g.rightShoulder.pressed) b |= 1u<<7;
    if (g.leftTrigger.value >= 0.5) b |= 1u<<8;
    if (g.rightTrigger.value >= 0.5) b |= 1u<<9;
    if (g.dpad.up.pressed) b |= 1u<<10;
    if (g.dpad.right.pressed) b |= 1u<<11;
    if (g.dpad.down.pressed) b |= 1u<<12;
    if (g.dpad.left.pressed) b |= 1u<<13;
    if (g.leftThumbstickButton.pressed) b |= 1u<<14;
    if (g.rightThumbstickButton.pressed) b |= 1u<<15;
    sticks[0] = g.rightThumbstick.xAxis.value;
    sticks[1] = -g.rightThumbstick.yAxis.value;
    sticks[2] = g.leftThumbstick.xAxis.value;
    sticks[3] = -g.leftThumbstick.yAxis.value;
    *has_select = g.buttonOptions != nil;
    return b;
}
