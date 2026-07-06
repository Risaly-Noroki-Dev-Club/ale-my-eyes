package com.alemyeyes.foreground;

import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.app.Service;
import android.content.Context;
import android.content.Intent;
import android.content.pm.ServiceInfo;
import android.os.Build;
import android.os.IBinder;
import android.os.PowerManager;
import android.util.Log;

/**
 * 前台服务 — 保持 Ale, My Eyes! 在后台持续运行（监听语音、执行自动化）。
 *
 * 在 Android 8.0+ 上，后台服务会在几分钟内被系统杀死。
 * 前台服务 + 持久通知是保持应用存活的标准做法。
 */
public class AleForegroundService extends Service {

    private static final String TAG = "AleForegroundService";
    private static final String CHANNEL_ID = "ale_my_eyes_service";
    private static final int NOTIFICATION_ID = 1001;
    private static final String WAKELOCK_TAG = "AleMyEyes::AudioCapture";

    private static AleForegroundService instance;
    private PowerManager.WakeLock wakeLock;

    public static AleForegroundService getInstance() {
        return instance;
    }

    @Override
    public void onCreate() {
        super.onCreate();
        instance = this;
        createNotificationChannel();
    }

    @Override
    public int onStartCommand(Intent intent, int flags, int startId) {
        String action = intent != null ? intent.getAction() : null;

        if ("STOP".equals(action)) {
            stopForegroundAndSelf();
            return START_NOT_STICKY;
        }

        // 启动前台通知
        Notification notification = buildNotification();
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            startForeground(
                NOTIFICATION_ID,
                notification,
                ServiceInfo.FOREGROUND_SERVICE_TYPE_MICROPHONE
                    | ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PLAYBACK
            );
        } else {
            startForeground(NOTIFICATION_ID, notification);
        }

        // 获取 WakeLock 保持 CPU 活跃（用于 VAD 持续监听）
        acquireWakeLock();

        Log.i(TAG, "Foreground service started");
        return START_STICKY;
    }

    @Override
    public IBinder onBind(Intent intent) {
        return null;
    }

    @Override
    public void onDestroy() {
        releaseWakeLock();
        instance = null;
        Log.i(TAG, "Foreground service destroyed");
        super.onDestroy();
    }

    /**
     * 更新通知栏文字（如状态变化时）
     */
    public void updateNotification(String statusText) {
        NotificationManager nm = getSystemService(NotificationManager.class);
        if (nm != null) {
            Notification notification = buildNotification(statusText);
            nm.notify(NOTIFICATION_ID, notification);
        }
    }

    /**
     * 获取 WakeLock 状态
     */
    public boolean isWakeLockHeld() {
        return wakeLock != null && wakeLock.isHeld();
    }

    // ── Private helpers ──────────────────────────────────────────

    private void createNotificationChannel() {
        NotificationChannel channel = new NotificationChannel(
            CHANNEL_ID,
            "Ale, My Eyes! 服务",
            NotificationManager.IMPORTANCE_LOW
        );
        channel.setDescription("保持 Ale, My Eyes! 在后台持续监听语音指令");
        channel.setShowBadge(false);
        channel.setSound(null, null);
        channel.enableVibration(false);

        NotificationManager nm = getSystemService(NotificationManager.class);
        if (nm != null) {
            nm.createNotificationChannel(channel);
        }
    }

    private Notification buildNotification() {
        return buildNotification("正在监听语音指令...");
    }

    private Notification buildNotification(String contentText) {
        // 点击通知打开主 Activity
        Intent launchIntent = getPackageManager().getLaunchIntentForPackage(getPackageName());
        PendingIntent pendingIntent = PendingIntent.getActivity(
            this, 0, launchIntent,
            PendingIntent.FLAG_UPDATE_CURRENT | PendingIntent.FLAG_IMMUTABLE
        );

        // 停止服务的 Action
        Intent stopIntent = new Intent(this, AleForegroundService.class);
        stopIntent.setAction("STOP");
        PendingIntent stopPendingIntent = PendingIntent.getService(
            this, 1, stopIntent,
            PendingIntent.FLAG_UPDATE_CURRENT | PendingIntent.FLAG_IMMUTABLE
        );

        Notification.Builder builder = new Notification.Builder(this, CHANNEL_ID)
            .setContentTitle("Ale, My Eyes!")
            .setContentText(contentText)
            .setSmallIcon(android.R.drawable.ic_btn_speak_now)
            .setContentIntent(pendingIntent)
            .setOngoing(true)
            .addAction(
                new Notification.Action.Builder(
                    null,
                    "停止",
                    stopPendingIntent
                ).build()
            );

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            builder.setForegroundServiceBehavior(
                Notification.FOREGROUND_SERVICE_IMMEDIATE
            );
        }

        return builder.build();
    }

    private void acquireWakeLock() {
        if (wakeLock != null && wakeLock.isHeld()) {
            return;
        }

        PowerManager pm = getSystemService(PowerManager.class);
        if (pm != null) {
            wakeLock = pm.newWakeLock(
                PowerManager.PARTIAL_WAKE_LOCK,
                WAKELOCK_TAG
            );
            wakeLock.acquire();
            Log.i(TAG, "WakeLock acquired");
        }
    }

    private void releaseWakeLock() {
        if (wakeLock != null && wakeLock.isHeld()) {
            wakeLock.release();
            wakeLock = null;
            Log.i(TAG, "WakeLock released");
        }
    }

    private void stopForegroundAndSelf() {
        releaseWakeLock();
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) {
            stopForeground(STOP_FOREGROUND_REMOVE);
        } else {
            stopForeground(true);
        }
        stopSelf();
    }

    // ── Static helpers for Rust JNI ──────────────────────────────

    /**
     * 启动前台服务
     */
    public static void startService(Context context) {
        Intent intent = new Intent(context, AleForegroundService.class);
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            context.startForegroundService(intent);
        } else {
            context.startService(intent);
        }
    }

    /**
     * 停止前台服务
     */
    public static void stopService(Context context) {
        Intent intent = new Intent(context, AleForegroundService.class);
        intent.setAction("STOP");
        context.startService(intent);
    }
}
