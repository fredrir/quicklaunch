#!/usr/bin/env python3
import json
import os
import sys


if os.environ.get("QUICKLAUNCH_PLUGIN_PROTOCOL") != "1":
    print("unsupported quicklaunch plugin protocol", file=sys.stderr)
    raise SystemExit(2)

json.dump(
    [
        {
            "id": "quicklaunch-repository",
            "name": "Quicklaunch repository",
            "generic_name": "Open in browser",
            "keywords": ["quicklaunch", "source", "git"],
            "icon": "internet-web-browser",
            "command": [
                "xdg-open",
                "https://github.com/fredrir/quicklaunch",
            ],
        }
    ],
    sys.stdout,
)
