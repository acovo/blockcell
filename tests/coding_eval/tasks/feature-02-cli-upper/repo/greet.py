import argparse

def main(argv=None):
    parser = argparse.ArgumentParser()
    parser.add_argument("name")
    args = parser.parse_args(argv)
    return f"Hello, {args.name}!"
