# Synthetic board fixture

`demo.json` is an explicit `roses-board-view-1` document with 49 known cells and
three invented cards. Its names, statistics and terrain arrangement are original
test data, not extracted retail records or implemented gameplay.

```powershell
.\target\release\roses-viewer.exe --snapshot examples/viewer/demo.json
.\target\release\roses-viewer.exe --snapshot examples/viewer/demo.json --check
```

The same fixture is embedded for the default asset-free demo. Cell selection is
presentation only. See `crates/roses-viewer/README.md` for the producer API and
strict saved-capture adapter.
