package org.bibleapp.reader

import android.os.Bundle
import androidx.activity.enableEdgeToEdge
import java.io.File

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    // Must run before super.onCreate: super starts the Tauri/Rust runtime,
    // whose setup opens the corpus. The file has to be in place by then.
    stageCorpus()
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
  }

  /**
   * Copies the bundled ~159 MB SQLite corpus out of the APK's read-only asset
   * bundle into the app's private data directory, where rusqlite can open it
   * (WAL + migrations both write, so a writable path is required).
   *
   * Android assets are not real files — they live inside the APK zip and are
   * reachable only through the AssetManager, which is why this copy cannot be
   * done on the Rust side. The Rust code opens the result at
   * `applicationInfo.dataDir/corpus.db`; see `mobile_corpus_path` in
   * `apps/desktop/src/lib.rs`. Keep the target path in sync with it.
   *
   * Idempotent: on every launch after the first the file already exists and
   * this returns immediately.
   */
  private fun stageCorpus() {
    val target = File(applicationInfo.dataDir, "corpus.db")
    if (target.exists() && target.length() > 0L) return

    // Stream the copy: the asset is large and, if stored compressed, cannot be
    // opened as a file descriptor, but assets.open() streams it fine.
    assets.open("corpus.db").use { input ->
      target.outputStream().use { output ->
        input.copyTo(output, bufferSize = 1 shl 20)
      }
    }
  }
}
