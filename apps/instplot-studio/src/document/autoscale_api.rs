use super::*;

impl FigureDocument {
    pub fn refresh_autoscale(&mut self) -> Result<(), String> {
        let mut candidate = self.project.clone();
        refresh_active_autoscales(&mut candidate)?;
        candidate.validate().map_err(|error| error.to_string())?;
        self.project = candidate;
        Ok(())
    }
}
