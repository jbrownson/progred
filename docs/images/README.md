# README screenshots

These are captures of the actual editor projection, not mockups. Regenerate
them, along with the website's copies and its home page stills, from the
repository root on macOS:

```sh
make images
```

It needs `rsvg-convert`, supplied by librsvg. The capture runs without
launching the app. It waits for the CAM preview's mesh and progressive
implicit jobs to finish, including the final depth refinement, before saving
the cube image.
The framing and playback position are ordinary view state set by the capture.

The tree scene recreates Bret Victor's
[Inventing on Principle](https://worrydream.com/InventingOnPrinciple/) demo;
the main README includes that credit beside the image.
The logo is referenced directly from `assets/progred-icon.svg`, the source
artwork used to generate the application icons.
