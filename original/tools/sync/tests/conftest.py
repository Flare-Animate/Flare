"""Collection guard for the sync-agent tests.

`upstream_agent` needs Python 3.11+. That used to be enforced with `sys.exit()` inside the
module, which pytest turns into an INTERNALERROR during collection — and an aborted session
also hides every other test being collected, including tools/flash/tests. One module's
interpreter requirement must not do that.

So: skip collection here under an older interpreter, and record the skip visibly in
`test_interpreter_supports_the_sync_suite`. A skip is not a pass; it is reported as one.
`upstream_agent` now raises ImportError instead of exiting for the same reason.
"""

import sys

collect_ignore_glob = []
if sys.version_info < (3, 11):
    collect_ignore_glob = ["test_upstream_agent.py"]
