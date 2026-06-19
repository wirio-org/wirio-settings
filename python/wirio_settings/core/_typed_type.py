import typing
from collections.abc import (
    Hashable,
    Mapping,
    Sequence,
)
from typing import Any, Final, final, override


@final
class TypedType(Hashable):
    """Version of :class:`type` that takes into account generic parameters."""

    _annotation: Final[Any]
    _origin: Final[Any]
    _args: Final[tuple[Any, ...]]

    def __init__(
        self,
        annotation: Any,  # noqa: ANN401
    ) -> None:
        self._annotation = annotation
        origin = typing.get_origin(annotation)
        has_generics = origin is not None

        if not has_generics:
            self._origin = annotation
            self._args = ()
            return

        self._origin = origin
        self._args = typing.get_args(annotation)

    @classmethod
    def from_type(cls, type_: type) -> "TypedType":
        return cls(type_)

    @property
    def annotation(
        self,
    ) -> Any:  # noqa: ANN401
        """Get the original type annotation from which this `TypedType` was created."""
        return self._annotation

    @property
    def args(self) -> tuple[Any, ...]:
        """Get the generic type arguments for this type."""
        return self._args

    @property
    def is_mapping(self) -> bool:
        """Get a value indicating whether the current type is a mapping type."""
        return issubclass(self._origin, Mapping)

    @property
    def is_sequence(self) -> bool:
        """Get a value indicating whether the current type is a collection type."""
        if self._origin in [str, bytes]:
            return False

        return issubclass(self._origin, Sequence)

    def to_type(self) -> type:
        return self._origin

    def _create_representation(
        self,
        origin: Any,  # noqa: ANN401
        args: tuple[Any, ...],
    ) -> str:
        args_representation = ""

        if len(args) > 0:
            for arg in args:
                arg_origin = typing.get_origin(arg)
                arg_args = typing.get_args(arg)
                has_generics = arg_origin is not None

                if has_generics:
                    args_representation += self._create_representation(
                        arg_origin, arg_args
                    )
                else:
                    args_representation += f"{arg.__module__}.{arg.__qualname__}"

                if arg != args[-1]:
                    args_representation += ", "

        if len(args_representation) > 0:
            args_representation = f"[{args_representation}]"

        return f"{origin.__module__}.{origin.__qualname__}{args_representation}"

    @override
    def __repr__(self) -> str:
        return self._create_representation(self._origin, self._args)

    @override
    def __hash__(self) -> int:
        return hash(self._origin) ^ hash(self._args)

    @override
    def __eq__(self, value: object) -> bool:
        if not isinstance(value, TypedType):
            return NotImplemented

        return self._origin == value._origin and self._args == value._args
