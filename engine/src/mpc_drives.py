"""Lists the drives on an MPC as Akai's remote tool finds them, for the workbench to read.

Run as: mpc_drives.py TOOLS_FOLDER ADDRESS

Akai's own code reaches the MPC and tells a built-in drive from one that can be pulled out. Nothing is
written anywhere. The last line printed is the list.
"""
import json
import os
import sys

sys.path.insert(0, sys.argv[1])
import mpc_remote  # noqa: E402  (found in the folder named above)

LISTED_AFTER = "@@DRIVES "
SSH_USER = "root"
SSH_PORT = 22
COULD_NOT_REACH = 2


def drives_found(address):
    """The MPC's drives, or None once Akai's tool has said why it could not be reached."""
    remote = mpc_remote.Remote(address, SSH_USER, SSH_PORT, os.environ.get("MPC_SSH_PASSWORD"))
    try:
        mpc_remote.connect(remote)
        return mpc_remote.discover_drives(remote)
    except mpc_remote.SshError as error:
        mpc_remote.explain(error, address, SSH_PORT, remote.password is not None)
        return None
    finally:
        remote.close()


def as_listed(drive):
    return {"name": drive.name, "mount": drive.mount, "internal": bool(drive.internal), "writable": bool(drive.writable)}


def main():
    drives = drives_found(sys.argv[2])
    if drives is None:
        return COULD_NOT_REACH
    print(LISTED_AFTER + json.dumps([as_listed(drive) for drive in drives]))
    return 0


if __name__ == "__main__":
    sys.exit(main())
