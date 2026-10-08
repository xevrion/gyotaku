# The text recognition plugin refers to ML Kit's Chinese, Devanagari, Japanese
# and Korean recognizers, which this app does not ship: it only asks for the
# Latin one. R8 stops on the missing classes unless told they are meant to be
# missing.
-dontwarn com.google.mlkit.vision.text.chinese.**
-dontwarn com.google.mlkit.vision.text.devanagari.**
-dontwarn com.google.mlkit.vision.text.japanese.**
-dontwarn com.google.mlkit.vision.text.korean.**
