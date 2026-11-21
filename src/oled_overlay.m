#import <AppKit/AppKit.h>
#include <stdint.h>

@interface OLEDOverlay : NSObject
@property(nonatomic, strong) NSPanel *shade;
@property(nonatomic, strong) id displayActivity;
- (BOOL)show;
- (void)hide;
@end

@implementation OLEDOverlay
- (BOOL)show {
    if (self.shade.visible) return YES;
    NSScreen *screen = NSScreen.mainScreen;
    if (!screen) return NO;

    NSPanel *shade = [[NSPanel alloc] initWithContentRect:screen.frame
        styleMask:NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel
        backing:NSBackingStoreBuffered defer:NO];
    shade.releasedWhenClosed = NO;
    shade.opaque = NO;
    shade.backgroundColor = [NSColor colorWithWhite:0 alpha:.38];
    shade.hasShadow = NO;
    shade.ignoresMouseEvents = YES;
    shade.hidesOnDeactivate = NO;
    shade.level = NSFloatingWindowLevel;
    shade.collectionBehavior = NSWindowCollectionBehaviorCanJoinAllSpaces
        | NSWindowCollectionBehaviorFullScreenAuxiliary | NSWindowCollectionBehaviorStationary;
    shade.title = @"OLED software shade";
    self.shade = shade;

    self.displayActivity = [NSProcessInfo.processInfo beginActivityWithOptions:NSActivityIdleDisplaySleepDisabled
        reason:@"Keep the display on during the OLED dimming stage"];
    [shade orderFrontRegardless];
    return shade.visible;
}

- (void)hide {
    [self.shade orderOut:nil];
    self.shade = nil;
    if (self.displayActivity) {
        [NSProcessInfo.processInfo endActivity:self.displayActivity];
        self.displayActivity = nil;
    }
}
@end

// Only the main thread owns AppKit state. The scheduler calls this without holding its mutex.
int oled_overlay_set_visible(int visible) {
    static OLEDOverlay *overlay;
    __block BOOL success = YES;
    void (^change)(void) = ^{
        if (visible) {
            if (!overlay) overlay = [OLEDOverlay new];
            success = [overlay show];
            if (!success) [overlay hide];
        } else {
            [overlay hide];
        }
    };
    if (NSThread.isMainThread) change();
    else dispatch_sync(dispatch_get_main_queue(), change);
    return success;
}

int64_t read_shade_delay(void) {
    NSUserDefaults *defaults = NSUserDefaults.standardUserDefaults;
    if ([defaults objectForKey:@"ShadeDelaySeconds"]) {
        return [defaults integerForKey:@"ShadeDelaySeconds"];
    }
    // Preserve the previously configured duration of the first stage.
    return [defaults integerForKey:@"CharacterDelaySeconds"];
}

void save_shade_delay(int64_t seconds) {
    [NSUserDefaults.standardUserDefaults setInteger:seconds forKey:@"ShadeDelaySeconds"];
}
