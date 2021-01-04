use std::fmt;

use crate::{
    func::{Func, FuncId},
    statement::StatementKind,
};

use liquid_rust_common::index::IndexMap;

pub struct Program<S> {
    functions: IndexMap<FuncId, Func<S>>,
}

impl<S> Program<S> {
    pub fn builder(functions_len: usize) -> ProgramBuilder<S> {
        ProgramBuilder {
            functions: IndexMap::from_raw((0..functions_len).map(|_| None).collect()),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (FuncId, &Func<S>)> {
        self.functions.iter()
    }

    pub fn get_func(&self, func_id: FuncId) -> &Func<S> {
        self.functions.get(func_id).unwrap()
    }
}

impl<S> fmt::Display for Program<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (func_id, func) in self.iter() {
            write!(f, "\n{}", func_id)?;

            if func.user_ty() {
                write!(f, ": {}", func.ty())?;
            }

            write!(f, " = fn(")?;

            let mut arguments = func.arguments();

            if let Some((argument, ty)) = arguments.next() {
                write!(f, "{}: {}", argument, ty)?;

                for (argument, ty) in arguments {
                    write!(f, ", {}: {}", argument, ty)?;
                }
            }

            writeln!(f, ") -> {} {{", func.return_ty())?;

            for (local, ty) in func.temporaries() {
                writeln!(f, "\t{}: {};", local, ty)?;
            }

            for (bb_id, bb) in func.bblocks() {
                writeln!(f, "\n\t{}: {{", bb_id)?;

                for statement in bb.statements() {
                    if !matches!(statement.kind, StatementKind::Noop) {
                        writeln!(f, "\t\t{};", statement)?;
                    }
                }

                writeln!(f, "\t\t{};", bb.terminator())?;

                writeln!(f, "\t}}")?;
            }

            writeln!(f, "}}")?;
        }

        Ok(())
    }
}

pub struct ProgramBuilder<S> {
    functions: IndexMap<FuncId, Option<Func<S>>>,
}

impl<S> ProgramBuilder<S> {
    pub fn add_func(&mut self, func_id: FuncId, func: Func<S>) -> bool {
        self.functions
            .get_mut(func_id)
            .unwrap()
            .replace(func)
            .is_some()
    }

    pub fn func_ids(&self) -> impl Iterator<Item = FuncId> {
        self.functions.keys()
    }

    pub fn build(self) -> Result<Program<S>, FuncId> {
        let mut functions = Vec::with_capacity(self.functions.len());

        for (func_id, func) in self.functions {
            functions.push(func.ok_or(func_id)?);
        }

        Ok(Program {
            functions: IndexMap::from_raw(functions),
        })
    }
}
