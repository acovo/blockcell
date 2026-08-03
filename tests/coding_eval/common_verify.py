import pathlib
import subprocess
import sys


def main() -> int:
    workspace = pathlib.Path(sys.argv[1]).resolve()
    completed = subprocess.run(
        [sys.executable, "-B", "-m", "unittest", "discover", "-s", str(workspace), "-p", "test_*.py", "-v"],
        cwd=workspace,
        check=False,
    )
    return completed.returncode


if __name__ == "__main__":
    raise SystemExit(main())
