#!/usr/bin/env python3
"""Simple log watcher that prints the last few lines when a .log file is modified.

Usage:
    python scripts/log_watcher.py [directory]

If no directory is given, it defaults to `flare/build`.

This script requires the `watchdog` package; install with:

    pip install watchdog

The autonomous chatmode is free to launch this script in a background terminal
and read its output to be notified of any changes to build/test log files.
"""

import sys
import time
import os

try:
    from watchdog.observers import Observer
    from watchdog.events import PatternMatchingEventHandler
except ImportError:
    print("Error: watchdog package not found. Install with `pip install watchdog`.")
    sys.exit(1)


class LogHandler(PatternMatchingEventHandler):
    def __init__(self, patterns):
        super().__init__(patterns=patterns)

    def on_modified(self, event):
        # only handle files, not directories
        if event.is_directory:
            return
        print(f"\n[log watcher] {event.src_path} modified")
        try:
            with open(event.src_path, 'r', encoding='utf-8', errors='ignore') as f:
                lines = f.readlines()
            # print the last 20 lines
            print(''.join(lines[-20:]))
        except Exception as e:
            print(f"Error reading log file: {e}")


def main():
    if len(sys.argv) > 1:
        path = sys.argv[1]
    else:
        path = os.path.join(os.getcwd(), 'flare', 'build')
    if not os.path.isdir(path):
        print(f"Directory {path} does not exist")
        sys.exit(1)

    patterns = ["*.log"]
    event_handler = LogHandler(patterns)
    observer = Observer()
    observer.schedule(event_handler, path, recursive=True)
    observer.start()
    print(f"Watching {path} for log changes...")
    try:
        while True:
            time.sleep(1)
    except KeyboardInterrupt:
        observer.stop()
    observer.join()


if __name__ == '__main__':
    main()
