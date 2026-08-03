from service import OrderService
def create_order(order_id, total): OrderService().create(order_id, total)
