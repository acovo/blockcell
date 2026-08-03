from models import user

def display_user(name, email):
    value = user(name, email)
    return f"{value['name'].strip().title()} <{value['email'].strip().lower()}>"
