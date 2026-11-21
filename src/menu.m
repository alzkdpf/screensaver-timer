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
