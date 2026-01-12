import matplotlib.pyplot as plt
import os
import sys
import subprocess
import datetime
import pandas as pd
import typing

def stop(code:int) -> typing.NoReturn:
    # os.system('cls' if os.name == 'nt' else 'clear')
    return sys.exit(code)

# Check for valid host format
try:
    bind_to = sys.argv[1].split(":")
    assert len(bind_to) == 2
    ip = bind_to[0].strip()
    ip = ip.replace("localhost", "127.0.0.1")
    ip_segments = ip.split(".")
    for segment in ip_segments:
        num = int(segment)
        assert num >= 0 and num <= 255
    port = int(bind_to[1].strip())
    assert port >=0 and port <= 2**16
except:
    print(f"Invalid arguments: {sys.argv}. Please provide a host IP and port of a mini-redis-server-rs instance to start this test.")
    print("Example: python test.py 127.0.0.1:3000")
    stop(1)

START_TIME = datetime.datetime.now()
LOCAL = ip == "127.0.0.1"

if LOCAL:
    print("Warning: Server specified on local machine, this can skew the results. Continue? (Y/n)")
    if input().strip().lower() != "y":
        print("Test cancelled")
        stop(1)


# subprocess.call(['cargo', 'test', '--workspace'])
