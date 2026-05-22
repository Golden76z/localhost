#!/usr/bin/env python3
"""CGI demo: session counter via PATH_INFO / query."""
import os
import sys

action = os.environ.get("QUERY_STRING", "").split("action=")[-1].split("&")[0]

print("Content-Type: text/plain\r")
print("\r", end="")

if action == "bump":
    print("visit count bumped (demo)")
elif action == "session":
    print(f"Method={os.environ.get('REQUEST_METHOD')} PATH_INFO={os.environ.get('PATH_INFO','')}")
else:
    print("Hello from Python CGI")
    print("PATH_INFO=", os.environ.get("PATH_INFO", ""))
