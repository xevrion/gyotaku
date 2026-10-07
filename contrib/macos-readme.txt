gyotaku for macOS (Apple Silicon)
=================================

Search every screenshot you have taken by the text inside it.

Getting started
---------------

The easiest way is the one-line install in a terminal:

    curl -fsSL https://raw.githubusercontent.com/xevrion/gyotaku/main/install.sh | sh

By hand:

1. Keep all the files together in one folder you won't delete:
     gyotaku-app                 the search window
     gyotaku                     the background reader and command line
     libonnxruntime.1.28.2.dylib Microsoft's ONNX Runtime, used to read text
2. Run gyotaku-app. The first time, it asks which folders to read
   (the Desktop is where macOS saves Cmd+Shift+3 screenshots) and
   whether to keep reading new ones in the background. Saying yes
   installs a launchd agent that starts it when you sign in.
3. Press Alt+Shift+S anywhere to open or close the search window.
   While it waits it sits in the menu bar instead of the Dock; its
   menu opens the window or settings, and quits it. Cmd+W and Cmd+Q
   in the window just put it away.

The first run downloads the OCR models once (about 22 MB). After that
everything happens on this computer, nothing is uploaded.

To remove it: turn off background reading in settings (Cmd+,), quit
from the menu bar icon, and delete the folder. The index lives in
~/Library/Application Support/gyotaku.

Problems and ideas: https://github.com/xevrion/gyotaku/issues
