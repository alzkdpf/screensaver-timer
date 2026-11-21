#import <AppKit/AppKit.h>
#include <math.h>

@interface OLEDOverlay : NSObject
@property(nonatomic, strong) NSPanel *shade;
@property(nonatomic, strong) NSPanel *character;
@property(nonatomic, strong) NSTimer *animation;
@property(nonatomic, strong) id displayActivity;
- (BOOL)show;
- (void)hide;
@end

static NSPanel *makePanel(NSRect rect, NSInteger level) {
    NSPanel *panel = [[NSPanel alloc] initWithContentRect:rect
        styleMask:NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel
        backing:NSBackingStoreBuffered defer:NO];
    panel.releasedWhenClosed = NO;
    panel.opaque = NO;
    panel.hasShadow = NO;
    panel.ignoresMouseEvents = YES;
    panel.hidesOnDeactivate = NO;
    panel.level = level;
    panel.collectionBehavior = NSWindowCollectionBehaviorCanJoinAllSpaces
        | NSWindowCollectionBehaviorFullScreenAuxiliary | NSWindowCollectionBehaviorStationary;
    return panel;
}

@implementation OLEDOverlay
- (BOOL)show {
    if (self.character.visible) return YES;
    NSScreen *screen = NSScreen.mainScreen;
    if (!screen) return NO;
    NSRect bounds = screen.frame;
    self.shade = makePanel(bounds, NSFloatingWindowLevel);
    self.shade.backgroundColor = [NSColor colorWithWhite:0 alpha:.38];
    self.shade.title = @"OLED software shade";
    self.character = makePanel(NSMakeRect(NSMinX(bounds), NSMaxY(bounds)-64, 178, 64),
                               NSFloatingWindowLevel + 1);
    NSString *name = @"OLED Protection Companion";
    self.character.title = name;
    self.character.backgroundColor = NSColor.clearColor;

    NSTextField *label = [NSTextField labelWithString:name];
    label.frame = NSMakeRect(8, 6, 162, 20);
    label.font = [NSFont systemFontOfSize:11 weight:NSFontWeightSemibold];
    label.alignment = NSTextAlignmentCenter;
    label.textColor = NSColor.whiteColor;
    label.backgroundColor = [NSColor colorWithWhite:.09 alpha:.95];
    label.drawsBackground = YES;
    [label setAccessibilityLabel:label.stringValue];
    NSImageView *icon = [[NSImageView alloc] initWithFrame:NSMakeRect(72, 26, 34, 34)];
    icon.image = [NSImage imageWithSystemSymbolName:@"hare.fill" accessibilityDescription:label.stringValue];
    icon.contentTintColor = [NSColor colorWithRed:.50 green:.91 blue:.73 alpha:1];
    [self.character.contentView addSubview:label];
    [self.character.contentView addSubview:icon];

    self.displayActivity = [NSProcessInfo.processInfo beginActivityWithOptions:NSActivityIdleDisplaySleepDisabled
        reason:@"Keep the display on during the OLED companion stage"];
    [self.shade orderFrontRegardless];
    [self.character orderFrontRegardless];

    double started = NSProcessInfo.processInfo.systemUptime;
    __weak NSPanel *weakCharacter = self.character;
    self.animation = [NSTimer timerWithTimeInterval:1./30. repeats:YES block:^(NSTimer *timer) {
        NSPanel *character = weakCharacter;
        if (!character) return;
        double elapsed = NSProcessInfo.processInfo.systemUptime - started;
        double width = MAX(1, NSWidth(bounds)-NSWidth(character.frame));
        double height = MAX(1, NSHeight(bounds)-NSHeight(character.frame));
        double pass = elapsed * 105 / width;
        int row = ((int)pass) % 24;
        double fraction = pass - floor(pass);
        double x = row % 2 ? 1 - fraction : fraction;
        // Sweep down and back up, so the animation repeats without a jump.
        double progress = (row + fraction) / 12.;
        double y = fabs(1 - progress);
        [character setFrameOrigin:NSMakePoint(NSMinX(bounds)+x*width, NSMinY(bounds)+y*height)];
        NSRect frame = icon.frame;
        frame.origin.y = 26 + 3 * (1 + sin(elapsed * 4 * M_PI));
        icon.frame = frame;
    }];
    [NSRunLoop.mainRunLoop addTimer:self.animation forMode:NSRunLoopCommonModes];
    return self.character.visible && self.shade.visible;
}

- (void)hide {
    [self.animation invalidate];
    self.animation = nil;
    [self.character orderOut:nil];
    [self.shade orderOut:nil];
    self.character = nil;
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

int64_t read_character_delay(void) {
    return [NSUserDefaults.standardUserDefaults integerForKey:@"CharacterDelaySeconds"];
}

void save_character_delay(int64_t seconds) {
    [NSUserDefaults.standardUserDefaults setInteger:seconds forKey:@"CharacterDelaySeconds"];
}
