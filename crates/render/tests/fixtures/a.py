@dataclass
class Order:
    """An order."""
    total: float = 90.0
    def apply(self, discount=None):  # twice?
        return self.total * (1 - discount) if discount else None
