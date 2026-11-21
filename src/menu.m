#import <AppKit/AppKit.h>
#include <stdint.h>

@interface TimerMenu : NSObject
@property(nonatomic, strong) NSStatusItem *item;
@property(nonatomic, strong) NSTimer *timer;
- (void)showWindow;
@end

@implementation TimerMenu
- (void)showWindow {
    [NSApp activateIgnoringOtherApps:YES];
    for (NSWindow *window in NSApp.windows) {
        if (![window.title isEqualToString:@"Screen Saver Timer"]) continue;
        if (window.isMiniaturized) [window deminiaturize:nil];
        [window makeKeyAndOrderFront:nil];
        break;
    }
}
@end

void *timer_menu_create(const void *state, int64_t (*read_seconds)(const void *)) {
    [NSApp setActivationPolicy:NSApplicationActivationPolicyRegular];
    NSDockTile *dockTile = NSApp.dockTile;
    TimerMenu *menu = [TimerMenu new];
    menu.item = [NSStatusBar.systemStatusBar statusItemWithLength:NSVariableStatusItemLength];
    NSStatusBarButton *button = menu.item.button;
    button.image = [NSImage imageWithSystemSymbolName:@"timer" accessibilityDescription:@"Screen Saver Timer"];
    button.imagePosition = NSImageLeading;
    button.font = [NSFont monospacedDigitSystemFontOfSize:[NSFont systemFontSize] weight:NSFontWeightRegular];
    button.toolTip = @"클릭하여 Screen Saver Timer 열기";
    button.target = menu;
    button.action = @selector(showWindow);
    __weak NSStatusBarButton *weakButton = button;
    menu.timer = [NSTimer timerWithTimeInterval:0.2 repeats:YES block:^(NSTimer *timer) {
        int64_t seconds = read_seconds(state);
        NSString *title = seconds == -1 ? @"" : @"—";
        if (seconds >= 0) {
            title = [NSString stringWithFormat:@"%02lld:%02lld:%02lld",
                     (long long)(seconds / 3600), (long long)(seconds / 60 % 60), (long long)(seconds % 60)];
        }
        if (![weakButton.title isEqualToString:title]) weakButton.title = title;
        NSString *badge = seconds == -1 ? nil : title;
        if (dockTile.badgeLabel != badge && ![dockTile.badgeLabel isEqualToString:badge]) {
            dockTile.badgeLabel = badge;
        }
    }];
    [NSRunLoop.mainRunLoop addTimer:menu.timer forMode:NSRunLoopCommonModes];
    [menu.timer fire];
    return (__bridge_retained void *)menu;
}

void timer_menu_destroy(void *handle) {
    TimerMenu *menu = (__bridge_transfer TimerMenu *)handle;
    [menu.timer invalidate];
    NSApp.dockTile.badgeLabel = nil;
    [NSStatusBar.systemStatusBar removeStatusItem:menu.item];
}

int timer_menu_shows_countdown(const void *handle) {
    TimerMenu *menu = (__bridge TimerMenu *)handle;
    NSString *title = menu.item.button.title;
    return [title containsString:@":"] && [NSApp.dockTile.badgeLabel isEqualToString:title];
}

int timer_dock_badge_is_clear(void) {
    return NSApp.dockTile.badgeLabel.length == 0;
}

@interface SaverObserver : NSObject
@property(nonatomic, assign) const void *state;
@property(nonatomic, assign) void (*event)(const void *, int);
- (void)started:(NSNotification *)notice;
- (void)stopped:(NSNotification *)notice;
@end

@implementation SaverObserver
- (void)started:(NSNotification *)notice {
    if (![notice.object isEqual:@"runFromPref"]) self.event(self.state, 1);
}
- (void)stopped:(NSNotification *)notice {
    if (![notice.object isEqual:@"runFromPref"]) self.event(self.state, 0);
}
@end

void *saver_observe(const void *state, void (*event)(const void *, int)) {
    SaverObserver *observer = [SaverObserver new];
    observer.state = state;
    observer.event = event;
    NSDistributedNotificationCenter *center = NSDistributedNotificationCenter.defaultCenter;
    [center addObserver:observer selector:@selector(started:) name:@"com.apple.screensaver.didstart"
                 object:nil suspensionBehavior:NSNotificationSuspensionBehaviorDeliverImmediately];
    for (NSString *name in @[@"com.apple.screensaver.willstop", @"com.apple.screensaver.didstop"]) {
        [center addObserver:observer selector:@selector(stopped:) name:name
                     object:nil suspensionBehavior:NSNotificationSuspensionBehaviorDeliverImmediately];
    }
    return (__bridge_retained void *)observer;
}

void saver_unobserve(void *handle) {
    SaverObserver *observer = (__bridge_transfer SaverObserver *)handle;
    [NSDistributedNotificationCenter.defaultCenter removeObserver:observer];
}

int64_t read_display_delay(void) {
    return [NSUserDefaults.standardUserDefaults integerForKey:@"DisplaySleepDelaySeconds"];
}

void save_display_delay(int64_t seconds) {
    [NSUserDefaults.standardUserDefaults setInteger:seconds forKey:@"DisplaySleepDelaySeconds"];
}
