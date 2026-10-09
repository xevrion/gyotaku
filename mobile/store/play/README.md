# Publishing gyotaku on Google Play

What is in this folder, and what is still needed that a file cannot provide.

## In this folder

| File | What it is |
|---|---|
| `listing.md` | App name, short and full description, release notes, category |
| `privacy-policy.md` | The privacy policy text. Needs a contact email filled in and a public URL |
| `console-answers.md` | Suggested answers for Data safety, content rating and the two permission declarations |
| `icon-512.png` | Store icon |
| `feature-graphic.png` | Feature graphic |
| `screenshots/` | Eight phone screenshots |
| `tools/` | The two scripts that drew the demo content and composed the screenshots |

The app bundle to upload is built with `flutter build appbundle --release` and lands in `build/app/outputs/bundle/release/app-release.aab`. It is signed with the key named in `android/key.properties`, which becomes the upload key.

## Decide before the first upload

These cannot be changed afterwards, or are not yours alone to decide.

1. **The package name.** It is `app.gyotaku.gyotaku`, which borrows the domain of the upstream project (gyotaku.app). A package name is permanent once published. Either get the upstream owner's agreement to use it, or change it to one under a name you control.
2. **The name and the icon.** "gyotaku" and the fish mark belong to the upstream project by Yash Bavadiya. The license (GPL-3.0-or-later) allows publishing the code. It does not by itself settle using the project's name and icon on a store listing. Ask first, or publish under a different name with a clear "based on gyotaku" credit.
3. **Play App Signing.** Play keeps the real signing key and you keep the upload key. Back up `gyotaku-release.jks` and its password before uploading anything.

## Steps in Play Console

1. Create the app: name, default language, app, free.
2. Set up the store listing from `listing.md` and the graphics here.
3. Host the privacy policy at a public URL and enter it.
4. Fill in Data safety, content rating, target audience and ads from `console-answers.md`.
5. Submit the photo permission and foreground service declarations, with the video.
6. Upload the app bundle to a closed testing track.
7. Personal developer accounts created since November 2023 must run a closed test with at least 12 testers for 14 days before applying for production.

## Known gaps

- The screenshots show the dark theme only.
- The app has been run on emulators of Android 9, 10, 13 and 17 and on one Pixel 6. Play's pre-launch report will exercise more devices.
- With only Read Bengali on, Hindi text is misread as Bangla. The Bengali reader's scores do not tell the two apart. Turning Read Devanagari on as well fixes it.
