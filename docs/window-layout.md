# Window layout configuration

Projects with a Loom UI can keep window snapping policy in:

```text
config/window-layout.json
```

The file is optional. Missing fields use the defaults shown below, and the file
is included automatically when the project is packaged:

```json
{
  "enabled": true,
  "snapDistance": 16,
  "detachDistance": 32,
  "gap": 0,
  "linkMovement": true,
  "viewerWidth": 740,
  "agentsWidth": 620,
  "viewerPanel": {
    "enabled": true,
    "snapOnOpen": true,
    "preferredSide": "left"
  },
  "viewerAgents": {
    "enabled": true,
    "snapOnOpen": true,
    "preferredSide": "right"
  }
}
```

Distances are logical pixels and are scaled for each window's display. Supported
sides are `origin`, `left`, `right`, `top`, and `bottom`. `origin` places the
moving window's top-left corner at the anchor window's `(0, 0)` screen position.

`viewerPanel` and `viewerAgents` are independent siblings of the Metal viewer.
Moving an unlinked window within `snapDistance` of an overlapping edge snaps and
links it. Moving it perpendicularly more than `detachDistance` breaks the link.
When `linkMovement` is enabled, moving or resizing the viewer carries both linked
windows with it.

`viewerWidth` and `viewerHeight` set the Metal viewer size; `agentsWidth` and
`agentsHeight` set the Agents window size. Heights default to 720 px for the
viewer and 760 px for Agents when omitted.
