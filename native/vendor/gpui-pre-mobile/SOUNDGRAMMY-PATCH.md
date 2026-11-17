# GPUI mobile compatibility

Source: [longbridge/gpui-mobile](https://github.com/longbridge/gpui-mobile/tree/7548ebc4b24f00494af9976af8852c7163ded9ce),
revision `7548ebc4b24f00494af9976af8852c7163ded9ce`. The upstream license
choices and notices are retained. The Java host already in `native/gen/android`
comes from the same revision.

The registry release pins GPUI 0.3.5, and this Git revision pins GPUI 0.3.10.
GPUI Kit 0.10.0 pins 0.3.12, so the local host uses GPUI and `gpui-pre-wgpu`
0.3.12 and passes the window's surface configuration during context creation.
The upstream offscreen demo integration, which requires a separate wgpu fork,
is omitted.

Remove the local override when the upstream host supports the GPUI release
used by Kit. Verify Android IME, Activity recreation, and surface recovery
alongside the Rust compile and APK checks.
