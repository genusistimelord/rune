use crate::compiling::assemble::prelude::*;

/// Compile `self`.
impl Assemble for ast::Path {
    fn assemble(&self, c: &mut Compiler<'_>, needs: Needs) -> CompileResult<()> {
        let span = self.span();
        log::trace!("Path => {:?}", c.source.source(span));

        if let Some(ast::PathKind::SelfValue) = self.as_kind() {
            let var = c.scopes.get_var("self", c.source_id, c.visitor, span)?;

            if !needs.value() {
                return Ok(());
            }

            var.copy(&mut c.asm, span, "self");
            return Ok(());
        }

        let named = c.convert_path_to_named(self)?;

        if let Needs::Value = needs {
            if let Some(local) = named.as_local() {
                if let Some(var) = c.scopes.try_get_var(local, c.source_id, c.visitor, span)? {
                    var.copy(&mut c.asm, span, format!("var `{}`", local));
                    return Ok(());
                }
            }
        }

        if let Some(meta) = c.try_lookup_meta(span, &named.item)? {
            c.compile_meta(&meta, span, needs)?;
            return Ok(());
        }

        if let (Needs::Value, Some(local)) = (needs, named.as_local()) {
            // light heuristics, treat it as a type error in case the
            // first character is uppercase.
            if !local.starts_with(char::is_uppercase) {
                return Err(CompileError::new(
                    span,
                    CompileErrorKind::MissingLocal {
                        name: local.to_owned(),
                    },
                ));
            }
        };

        return Err(CompileError::new(
            span,
            CompileErrorKind::MissingItem {
                item: named.item.clone(),
            },
        ));
    }
}