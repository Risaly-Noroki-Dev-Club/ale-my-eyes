package com.alemyeyes.automation;

import android.accessibilityservice.AccessibilityService;
import android.accessibilityservice.AccessibilityServiceInfo;
import android.accessibilityservice.GestureDescription;
import android.content.Intent;
import android.content.pm.PackageManager;
import android.graphics.Path;
import android.graphics.Rect;
import android.net.Uri;
import android.os.Bundle;
import android.view.KeyEvent;
import android.view.accessibility.AccessibilityEvent;
import android.view.accessibility.AccessibilityNodeInfo;
import android.app.Instrumentation;

import java.util.List;

/**
 * Ale, My Eyes! 无障碍服务
 * 提供 UI 自动化能力，包括点击、滚动、输入文本等操作
 */
public class AleAccessibilityService extends AccessibilityService {

    private static AleAccessibilityService instance;

    public static AleAccessibilityService getInstance() {
        return instance;
    }

    @Override
    public void onServiceConnected() {
        super.onServiceConnected();
        instance = this;

        // 配置无障碍服务
        AccessibilityServiceInfo info = getServiceInfo();
        if (info == null) {
            info = new AccessibilityServiceInfo();
        }
        info.eventTypes = AccessibilityEvent.TYPES_ALL_MASK;
        info.feedbackType = AccessibilityServiceInfo.FEEDBACK_GENERIC;
        info.flags = AccessibilityServiceInfo.FLAG_INCLUDE_NOT_IMPORTANT_VIEWS
                | AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS
                | AccessibilityServiceInfo.FLAG_REQUEST_ENHANCED_WEB_ACCESSIBILITY;
        info.notificationTimeout = 100;
        setServiceInfo(info);
    }

    @Override
    public void onAccessibilityEvent(AccessibilityEvent event) {
        // 可以在这里监听 UI 事件
    }

    @Override
    public void onInterrupt() {
        // 服务被中断
    }

    @Override
    public void onDestroy() {
        instance = null;
        super.onDestroy();
    }

    /**
     * 在指定坐标执行点击
     * @param x X 坐标
     * @param y Y 坐标
     * @param actionType 1=点击, 2=长按
     * @return 是否成功
     */
    public boolean performClick(float x, float y, int actionType) {
        Path clickPath = new Path();
        clickPath.moveTo(x, y);

        GestureDescription.Builder builder = new GestureDescription.Builder();
        long duration = (actionType == 2) ? 1000 : 100; // 长按 1 秒
        builder.addStroke(new GestureDescription.StrokeDescription(clickPath, 0, duration));

        GestureDescription gesture = builder.build();
        return dispatchGesture(gesture, null, null);
    }

    /**
     * 在指定区域执行滚动
     * @param x 起始 X 坐标
     * @param y 起始 Y 坐标
     * @param deltaX 水平滚动量
     * @param deltaY 垂直滚动量
     * @return 是否成功
     */
    public boolean performScroll(float x, float y, float deltaX, float deltaY) {
        Path scrollPath = new Path();
        scrollPath.moveTo(x, y);
        scrollPath.lineTo(x - deltaX, y - deltaY);

        GestureDescription.Builder builder = new GestureDescription.Builder();
        builder.addStroke(new GestureDescription.StrokeDescription(scrollPath, 0, 300));

        GestureDescription gesture = builder.build();
        return dispatchGesture(gesture, null, null);
    }

    /**
     * 输入文本到当前焦点输入框
     * @param text 要输入的文本
     */
    public void performTypeText(String text) {
        AccessibilityNodeInfo focusedNode = findFocus(AccessibilityNodeInfo.FOCUS_INPUT);
        if (focusedNode == null) {
            // 尝试查找任何可输入的节点
            focusedNode = findEditableNode();
        }

        if (focusedNode != null) {
            Bundle args = new Bundle();
            args.putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE, text);
            focusedNode.performAction(AccessibilityNodeInfo.ACTION_SET_TEXT, args);
            focusedNode.recycle();
        }
    }

    /**
     * 模拟按键
     * @param keyCode Android KeyEvent KeyCode
     * @param modifiers 修饰键列表（如 "ctrl", "alt"）
     */
    public void performKeyPress(int keyCode, String[] modifiers) {
        // 对于普通按键，直接 dispatch
        if (modifiers == null || modifiers.length == 0) {
            dispatchGestureForKey(keyCode);
        } else {
            // 对于组合键，使用 KeyEvent
            int metaState = 0;
            for (String modifier : modifiers) {
                switch (modifier.toLowerCase()) {
                    case "ctrl":
                    case "control":
                        metaState |= KeyEvent.META_CTRL_ON;
                        break;
                    case "alt":
                        metaState |= KeyEvent.META_ALT_ON;
                        break;
                    case "shift":
                        metaState |= KeyEvent.META_SHIFT_ON;
                        break;
                    case "meta":
                    case "super":
                    case "win":
                    case "cmd":
                    case "command":
                        metaState |= KeyEvent.META_META_ON;
                        break;
                }
            }

            KeyEvent downEvent = new KeyEvent(0, 0, KeyEvent.ACTION_DOWN, keyCode, 0, metaState);
            KeyEvent upEvent = new KeyEvent(0, 0, KeyEvent.ACTION_UP, keyCode, 0, metaState);
            Instrumentation inst = new Instrumentation();
            inst.sendKeySync(downEvent);
            inst.sendKeySync(upEvent);
        }
    }

    /**
     * 打开指定应用
     * @param packageName 包名或应用名
     * @return 是否成功
     */
    public boolean openApp(String packageName) {
        PackageManager pm = getPackageManager();
        Intent launchIntent = pm.getLaunchIntentForPackage(packageName);

        if (launchIntent == null) {
            // 尝试通过应用名查找
            launchIntent = findAppByName(pm, packageName);
        }

        if (launchIntent != null) {
            launchIntent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
            startActivity(launchIntent);
            return true;
        }
        return false;
    }

    /**
     * 关闭指定应用（返回桌面）
     * @param packageName 包名
     * @return 是否成功
     */
    public boolean closeApp(String packageName) {
        // Android 无法直接关闭其他应用，只能返回桌面
        return performGlobalAction(GLOBAL_ACTION_HOME);
    }

    /**
     * 打开 URL
     * @param url 要打开的 URL
     */
    public void openUrl(String url) {
        Intent intent = new Intent(Intent.ACTION_VIEW, Uri.parse(url));
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        startActivity(intent);
    }

    /**
     * 获取屏幕根节点
     * @return 根节点
     */
    public AccessibilityNodeInfo getRootNode() {
        return getRootInActiveWindow();
    }

    /**
     * 查找可编辑的输入节点
     */
    private AccessibilityNodeInfo findEditableNode() {
        AccessibilityNodeInfo root = getRootInActiveWindow();
        if (root == null) return null;
        return findEditableNodeRecursive(root);
    }

    private AccessibilityNodeInfo findEditableNodeRecursive(AccessibilityNodeInfo node) {
        if (node == null) return null;

        if (node.isEditable() && node.isVisibleToUser()) {
            return node;
        }

        for (int i = 0; i < node.getChildCount(); i++) {
            AccessibilityNodeInfo child = node.getChild(i);
            if (child != null) {
                AccessibilityNodeInfo result = findEditableNodeRecursive(child);
                if (result != null) {
                    return result;
                }
                child.recycle();
            }
        }
        return null;
    }

    private Intent findAppByName(PackageManager pm, String appName) {
        Intent mainIntent = new Intent(Intent.ACTION_MAIN, null);
        mainIntent.addCategory(Intent.CATEGORY_LAUNCHER);

        List<android.content.pm.ResolveInfo> apps = pm.queryIntentActivities(mainIntent, 0);
        for (android.content.pm.ResolveInfo app : apps) {
            String label = app.loadLabel(pm).toString();
            if (label.equalsIgnoreCase(appName) || label.toLowerCase().contains(appName.toLowerCase())) {
                return pm.getLaunchIntentForPackage(app.activityInfo.packageName);
            }
        }
        return null;
    }

    private void dispatchGestureForKey(int keyCode) {
        // 对于普通按键，尝试使用全局操作
        switch (keyCode) {
            case KeyEvent.KEYCODE_HOME:
                performGlobalAction(GLOBAL_ACTION_HOME);
                break;
            case KeyEvent.KEYCODE_BACK:
                performGlobalAction(GLOBAL_ACTION_BACK);
                break;
            case KeyEvent.KEYCODE_APP_SWITCH:
                performGlobalAction(GLOBAL_ACTION_RECENTS);
                break;
            default:
                // 其他按键需要通过 InputManager 或 Root 权限
                break;
        }
    }
}
