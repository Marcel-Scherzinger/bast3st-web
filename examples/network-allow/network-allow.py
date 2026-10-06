#!/usr/bin/env python3
from typing import TypedDict
import json


class AllowNetworkCmdInput(TypedDict):
    # not set for "debug" requests
    user: str | None
    # not set for "debug" requests
    slot: str | None
    # http, https, ...
    scheme: str
    # domain, ip
    host: str | None
    port: int | None
    path: str
    query: list[tuple[str, str]]
    # the original url
    url: str
    # request data like the json that should be sent
    data: dict


data: AllowNetworkCmdInput = json.loads(input())

if data["scheme"] == "https":
    exit(0)
else:
    exit(1)
