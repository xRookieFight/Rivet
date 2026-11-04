package dev.rivet;

import android.os.Bundle;
import androidx.core.view.WindowCompat;
import com.google.androidgamesdk.GameActivity;

// GameActivity loads librivet_android.so from the android.app.lib_name
// meta-data and calls android_main.
public class MainActivity extends GameActivity {
    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        // GameActivity goes edge to edge, but egui never hears about the insets,
        // so let the system keep the surface clear of the status and nav bars
        WindowCompat.setDecorFitsSystemWindows(getWindow(), true);
    }
}
