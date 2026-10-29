package is.eightb.aurora;

import android.app.Activity;
import android.graphics.Color;
import android.os.Build;
import android.os.Bundle;
import android.os.VibrationEffect;
import android.os.Vibrator;
import android.view.Window;
import android.view.WindowManager;
import android.webkit.JavascriptInterface;
import android.webkit.WebChromeClient;
import android.webkit.WebSettings;
import android.webkit.WebView;
import android.webkit.WebViewClient;

public class MainActivity extends Activity {
    private WebView webView;

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        Window w = getWindow();
        if (w != null) {
            w.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
            w.setStatusBarColor(Color.parseColor("#05060a"));
            w.setNavigationBarColor(Color.parseColor("#05060a"));
        }

        webView = new WebView(this);
        webView.setBackgroundColor(Color.parseColor("#05060a"));
        WebSettings settings = webView.getSettings();
        settings.setJavaScriptEnabled(true);
        settings.setDomStorageEnabled(true);
        settings.setMediaPlaybackRequiresUserGesture(false);
        settings.setAllowFileAccess(true);

        webView.setWebViewClient(new WebViewClient());
        webView.setWebChromeClient(new WebChromeClient());
        webView.addJavascriptInterface(new AuroraBridge(), "AuroraBridge");

        setContentView(webView);
        webView.loadUrl("file:///android_asset/index.html");
    }

    public final class AuroraBridge {
        @JavascriptInterface
        public String feelMeTonight() {
            try {
                Vibrator v = getSystemService(Vibrator.class);
                if (v != null && v.hasVibrator()) {
                    if (Build.VERSION.SDK_INT >= 26) {
                        v.vibrate(VibrationEffect.createWaveform(new long[]{0, 42, 60, 84}, -1));
                    }
                }
            } catch (Throwable ignored) {
            }
            return "AURORA:FEEL_ME_TONIGHT:0x62:432Hz:100GHz";
        }
    }
}
