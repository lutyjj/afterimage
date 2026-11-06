# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Stack

React with Vite and TypeScript, on the TypeScript client generated from the Smithy model (`gen/management-client`). The server serves the built files from `AFTERIMAGE_WEB_DIR`.

## Users

The person who runs an Afterimage server for a household or a small group: they set devices up, write screens and arrange what each device shows. They write screens at a desk, with the preview beside the editor, and claim devices on a phone while standing next to the display reading its code. Other people of the household sign in to glance at their devices.

## Product Purpose

Afterimage is a self-hosted server for TRMNL e-paper displays. It onboards devices, draws screens from the data the user points it at (Home Assistant, any JSON over HTTP), and serves each display the screen its playlist calls for. Success is a display that shows the right thing at the right time and needs no attention after setup.

## Positioning

One small server and one contract: the Smithy model defines every API, and screens are documents of generic components in TRMNL's own design, drawn in milliseconds without a browser. It runs no community recipes and keeps no history.

## Operating Context

- Claiming: a new device shows a six-character code; the user types it into the web UI.
- Screens: a YAML document (data sources, a title bar, components with Jinja templates), edited beside a preview drawn by the server at the device's real size, with what each source answered and every problem located in the document.
- Playlists: each device shows its screens in order, on chosen weekdays and between times of day; it wakes when its refresh interval ends or the set of current screens changes.
- Devices report battery, signal and firmware at each check-in; displays sleep between check-ins and cannot be woken from the server.
- Sign-in: a password for the first account, and an OIDC provider for everyone else when configured.

## Capabilities and Constraints

- Display models and palettes come from TRMNL's published lists; a device's display can be any model and palette TRMNL lists.
- Removing a device either returns it to its claim code or, with a reset, erases its Wi-Fi settings at its next check-in.
- Secrets for data sources live in the server's environment; the UI shows only their names.
- Scripts and agents act for an account with personal API tokens, created and revoked on the account page; a token's secret is shown once.
- Firmware comes from TRMNL's published releases. A device's board is matched by the model its firmware reports or chosen once; an install happens at the device's next check-in, and the check-in after it shows whether it worked.
- With an MQTT broker, every claimed device appears in Home Assistant with its battery, signal, last check-in, current screen and a firmware update it can install. The deployment sets the broker in its environment, or else the administrator (the first account) sets it on the Server page, where it takes effect at once.

## Brand Commitments

- Name: Afterimage: an e-paper screen keeps its last image with the power off.
- It must not feel like a toy or a gimmick: no mascots, cute illustration or playful copy. It is a calm tool.

## Evidence on Hand

No screenshots, customers or testimonials exist; future work must not invent them. Real screen previews come from the server.

## Product Principles

- The screens are the subject: the UI frames what a display will show and stays out of its way.
- Every state a device can be in is visible and named plainly: waiting to be claimed, showing a screen, nothing scheduled, not seen lately.
- Writing a screen is technical, everything around it is not.
- Nothing destructive happens without saying what it does to the device.
