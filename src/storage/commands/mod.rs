mod cat;
mod cp;
mod ls;
mod rm;

pub(crate) use cat::run as cat;
pub(crate) use cp::run as cp;
pub(crate) use ls::run as ls;
pub(crate) use rm::run as rm;
