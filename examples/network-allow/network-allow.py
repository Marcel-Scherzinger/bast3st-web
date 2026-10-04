#!/usr/bin/env python3
from typing import TypedDict
import json


class AllowNetworkCmdInput(TypedDict):
    scheme: str
    host: str | None
    port: int | None
    path: str
    query: list[tuple[str, str]]
    url: str
    data: dict


data: AllowNetworkCmdInput = json.loads(input())

if data["scheme"] == "https":
    exit(0)
else:
    exit(42)
