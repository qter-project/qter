use std::{error::Error, fmt::Display};

use interpreter::puzzle_states::RobotLike;
use puzzle_theory::permutations::{Algorithm, Permutation, PermutationGroup};

pub struct CaptureCubeState<T, F>(T, F);

impl<T: RobotLike, F: FnMut(&Permutation)> RobotLike for CaptureCubeState<T, F> {
    type InitializationArg = (T::InitializationArg, F);
    type Error = T::Error;

    async fn initialize(
        perm_group: std::sync::Arc<PermutationGroup>,
        (args, cb): Self::InitializationArg,
    ) -> Result<Self, Self::Error> {
        let mut this = Self(T::initialize(perm_group, args).await?, cb);
        this.1(&Permutation::identity());
        Ok(this)
    }

    async fn compose_into(&mut self, alg: &Algorithm) -> Result<(), Self::Error> {
        self.0.compose_into(alg).await
    }

    async fn take_picture(&mut self) -> Result<&Permutation, Self::Error> {
        let perm = self.0.take_picture().await?;
        self.1(perm);
        Ok(perm)
    }

    async fn solve(&mut self) -> Result<(), Self::Error> {
        self.0.solve().await
    }

    async fn compose_perm(&mut self, perm: &Permutation) -> Result<(), Self::Error> {
        self.0.compose_perm(perm).await
    }
}

#[derive(Debug)]
pub enum Either<T, U> {
    Left(T),
    Right(U),
}

impl<T: RobotLike, U: RobotLike> RobotLike for Either<T, U> {
    type InitializationArg = Either<T::InitializationArg, U::InitializationArg>;
    type Error = Either<T::Error, U::Error>;

    async fn initialize(
        perm_group: std::sync::Arc<PermutationGroup>,
        args: Self::InitializationArg,
    ) -> Result<Self, Self::Error> {
        Ok(match args {
            Either::Left(args) => Self::Left(
                T::initialize(perm_group, args)
                    .await
                    .map_err(Either::Left)?,
            ),
            Either::Right(args) => Self::Right(
                U::initialize(perm_group, args)
                    .await
                    .map_err(Either::Right)?,
            ),
        })
    }

    async fn compose_into(&mut self, alg: &Algorithm) -> Result<(), Self::Error> {
        match self {
            Self::Left(inner) => inner.compose_into(alg).await.map_err(Either::Left),
            Self::Right(inner) => inner.compose_into(alg).await.map_err(Either::Right),
        }
    }

    async fn take_picture(&mut self) -> Result<&Permutation, Self::Error> {
        match self {
            Self::Left(inner) => inner.take_picture().await.map_err(Either::Left),
            Self::Right(inner) => inner.take_picture().await.map_err(Either::Right),
        }
    }

    async fn solve(&mut self) -> Result<(), Self::Error> {
        match self {
            Self::Left(inner) => inner.solve().await.map_err(Either::Left),
            Self::Right(inner) => inner.solve().await.map_err(Either::Right),
        }
    }

    async fn compose_perm(&mut self, perm: &Permutation) -> Result<(), Self::Error> {
        match self {
            Self::Left(inner) => inner.compose_perm(perm).await.map_err(Either::Left),
            Self::Right(inner) => inner.compose_perm(perm).await.map_err(Either::Right),
        }
    }
}

impl<T: Display, U: Display> Display for Either<T, U> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Left(inner) => inner.fmt(f),
            Self::Right(inner) => inner.fmt(f),
        }
    }
}

#[allow(deprecated)]
impl<T: Error, U: Error> Error for Either<T, U> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Left(inner) => inner.source(),
            Self::Right(inner) => inner.source(),
        }
    }

    fn description(&self) -> &str {
        match self {
            Self::Left(inner) => inner.description(),
            Self::Right(inner) => inner.description(),
        }
    }

    fn cause(&self) -> Option<&dyn Error> {
        match self {
            Self::Left(inner) => inner.cause(),
            Self::Right(inner) => inner.cause(),
        }
    }
}
