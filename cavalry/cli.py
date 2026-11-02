import argparse
import sys

from cavalry.records import RecordError, capture, read_records, render_list, render_proposals


def build_parser():
    parser = argparse.ArgumentParser(
        prog="cavalry",
        description="Record the rescues only a human could make.",
    )
    sub = parser.add_subparsers(dest="command", required=True)

    capture_parser = sub.add_parser("capture", help="Append one validated record.")
    capture_parser.add_argument("input_file", help="JSON file holding one record.")
    capture_parser.add_argument("records_file", help="Records file to append to.")

    list_parser = sub.add_parser("list", help="Print the records with their classes.")
    list_parser.add_argument("records_file", help="Records file to read.")

    propose_parser = sub.add_parser(
        "propose", help="Print drafted proposals for the classes present."
    )
    propose_parser.add_argument("records_file", help="Records file to read.")
    return parser


def main(argv=None):
    args = build_parser().parse_args(argv)
    try:
        if args.command == "capture":
            stored = capture(args.input_file, args.records_file)
            sys.stdout.write(f"stored {stored['id']} as {stored['rescue_class']}\n")
        elif args.command == "list":
            sys.stdout.write(render_list(read_records(args.records_file)))
        elif args.command == "propose":
            sys.stdout.write(render_proposals(read_records(args.records_file)))
    except (RecordError, OSError) as exc:
        sys.stderr.write(f"error: {exc}\n")
        return 2
    return 0
