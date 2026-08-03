import json
from service import execute
def response(value): return json.dumps(execute(value), sort_keys=True)
