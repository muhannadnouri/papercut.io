# Libtashkeel / ORT compatibility

The isolated ORT migration proved same-process feasibility, but its local
Libtashkeel adaptation is not retained in the app. Papercut uses published
`libtashkeel_base` 1.5.0 and its compatible ORT `2.0.0-rc.1` bindings. The app
has no FastEmbed dependency or local Libtashkeel override.

This experiment builds separately with FastEmbed 7.1.0 and ORT `rc.13`.
Its own manifest and lockfile do not change the app dependency graph. Earlier
adaptation sources and exact probe reports remain in private archives;
there is no maintained fork in this checkout. Future same-process integration
needs an upstream-supported compatible dependency route and fresh validation.

Runtime feasibility does not establish a relevance win or authorize optional
hybrid search. Stage 7 remains on hold.
