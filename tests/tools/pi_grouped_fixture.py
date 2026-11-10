"""Drive the pinned Pi writer for the grouped Rust qualification test."""
from pathlib import Path
import select
import sys

from pi_producer_support import PausedProducer, produce, verify_source


def main():
    source_arg, root_arg, action, *pause = sys.argv[1:]
    source = verify_source(source_arg)
    root = Path(root_arg)
    if not pause:
        print(produce(source, root, action)['file'])
        return
    with PausedProducer(source, root, action, pause[0]) as writer:
        print(pause[0], flush=True)
        if not select.select([sys.stdin], [], [], 20)[0] or sys.stdin.readline() != '\n':
            raise RuntimeError('The grouped scanner did not release the paused fixture')
        print(writer.resume()['file'])


if __name__ == '__main__':
    main()
