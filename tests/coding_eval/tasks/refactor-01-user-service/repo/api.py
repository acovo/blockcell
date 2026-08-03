from service import display_user

def get_user_label(payload):
    return display_user(payload["name"], payload["email"])
