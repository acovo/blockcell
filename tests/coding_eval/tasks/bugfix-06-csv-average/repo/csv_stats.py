def average_csv(text):
    values = [float(line) for line in text.splitlines()]
    return sum(values) / len(values)
