package com.alemyeyes.automation;

import android.accessibilityservice.AccessibilityService;
import android.content.ContentResolver;
import android.content.Context;
import android.content.Intent;
import android.database.Cursor;
import android.net.Uri;
import android.os.Environment;
import android.provider.DocumentsContract;
import android.provider.MediaStore;
import android.provider.Settings;

import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.IOException;
import java.nio.channels.FileChannel;

/**
 * JNI 桥接类
 * 提供给 Rust 侧调用的 Java 接口
 */
public class AutomationBridge {

    private static final AutomationBridge instance = new AutomationBridge();
    private Context appContext;

    private AutomationBridge() {}

    public static AutomationBridge getInstance() {
        return instance;
    }

    /**
     * 设置应用上下文（在 JNI_OnLoad 或 Activity onCreate 时调用）
     */
    public void setContext(Context context) {
        this.appContext = context.getApplicationContext();
    }

    /**
     * 检查无障碍服务是否已启用
     */
    public boolean isAccessibilityServiceEnabled() {
        return AleAccessibilityService.getInstance() != null;
    }

    /**
     * 跳转到无障碍服务设置页面
     */
    public void openAccessibilitySettings() {
        if (appContext != null) {
            Intent intent = new Intent(Settings.ACTION_ACCESSIBILITY_SETTINGS);
            intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
            appContext.startActivity(intent);
        }
    }

    /**
     * 在指定坐标执行点击
     */
    public boolean performClick(double x, double y, int actionType) {
        AleAccessibilityService service = AleAccessibilityService.getInstance();
        if (service != null) {
            return service.performClick((float) x, (float) y, actionType);
        }
        return false;
    }

    /**
     * 在指定区域执行滚动
     */
    public boolean performScroll(double x, double y, double deltaX, double deltaY) {
        AleAccessibilityService service = AleAccessibilityService.getInstance();
        if (service != null) {
            return service.performScroll((float) x, (float) y, (float) deltaX, (float) deltaY);
        }
        return false;
    }

    /**
     * 输入文本
     */
    public void performTypeText(String text) {
        AleAccessibilityService service = AleAccessibilityService.getInstance();
        if (service != null) {
            service.performTypeText(text);
        }
    }

    /**
     * 模拟按键
     */
    public void performKeyPress(int keyCode, String[] modifiers) {
        AleAccessibilityService service = AleAccessibilityService.getInstance();
        if (service != null) {
            service.performKeyPress(keyCode, modifiers);
        }
    }

    /**
     * 打开指定应用
     */
    public boolean openApp(String packageName) {
        AleAccessibilityService service = AleAccessibilityService.getInstance();
        if (service != null) {
            return service.openApp(packageName);
        }
        return false;
    }

    /**
     * 关闭指定应用
     */
    public boolean closeApp(String packageName) {
        AleAccessibilityService service = AleAccessibilityService.getInstance();
        if (service != null) {
            return service.closeApp(packageName);
        }
        return false;
    }

    /**
     * 打开 URL
     */
    public void openUrl(String url) {
        AleAccessibilityService service = AleAccessibilityService.getInstance();
        if (service != null) {
            service.openUrl(url);
        }
    }

    /**
     * 执行文件操作
     * @param operation 操作类型: 0=创建, 1=删除, 2=移动, 3=复制, 4=重命名
     * @param path 文件路径
     * @param target 目标路径（移动/复制/重命名时使用）
     * @return 是否成功
     */
    public boolean performFileOperation(int operation, String path, String target) {
        try {
            File file = new File(path);
            File targetFile = (target != null && !target.isEmpty()) ? new File(target) : null;

            switch (operation) {
                case 0: // 创建
                    if (path.endsWith("/") || path.endsWith("\\")) {
                        return file.mkdirs();
                    } else {
                        File parent = file.getParentFile();
                        if (parent != null && !parent.exists()) {
                            parent.mkdirs();
                        }
                        return file.createNewFile();
                    }
                case 1: // 删除
                    return deleteRecursive(file);
                case 2: // 移动
                    if (targetFile != null) {
                        return file.renameTo(targetFile);
                    }
                    return false;
                case 3: // 复制
                    if (targetFile != null) {
                        copyFile(file, targetFile);
                        return true;
                    }
                    return false;
                case 4: // 重命名
                    if (targetFile != null) {
                        return file.renameTo(targetFile);
                    }
                    return false;
                default:
                    return false;
            }
        } catch (IOException e) {
            e.printStackTrace();
            return false;
        }
    }

    private boolean deleteRecursive(File fileOrDirectory) {
        if (fileOrDirectory.isDirectory()) {
            File[] children = fileOrDirectory.listFiles();
            if (children != null) {
                for (File child : children) {
                    deleteRecursive(child);
                }
            }
        }
        return fileOrDirectory.delete();
    }

    private void copyFile(File source, File dest) throws IOException {
        if (source.isDirectory()) {
            if (!dest.exists()) {
                dest.mkdirs();
            }
            String[] children = source.list();
            if (children != null) {
                for (String child : children) {
                    copyFile(new File(source, child), new File(dest, child));
                }
            }
        } else {
            File parent = dest.getParentFile();
            if (parent != null && !parent.exists()) {
                parent.mkdirs();
            }
            try (FileInputStream inStream = new FileInputStream(source);
                 FileOutputStream outStream = new FileOutputStream(dest);
                 FileChannel inChannel = inStream.getChannel();
                 FileChannel outChannel = outStream.getChannel()) {
                inChannel.transferTo(0, inChannel.size(), outChannel);
            }
        }
    }
}
