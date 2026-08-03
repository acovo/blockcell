from store import ORDERS

class OrderService:
    def create(self, order_id, total): ORDERS[order_id] = total
    def total(self, order_id): return ORDERS[order_id]
