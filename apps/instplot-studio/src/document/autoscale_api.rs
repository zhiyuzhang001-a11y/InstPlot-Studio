use super::*;

impl FigureDocument {
    pub fn refresh_autoscale(&mut self) -> Result<(), String> {
        let mut candidate = self.project.clone();
        apply_autoscale(&mut candidate, AxisDimension::X)?;
        apply_autoscale(&mut candidate, AxisDimension::Y)?;
        candidate.validate().map_err(|error| error.to_string())?;
        self.project = candidate;
        Ok(())
    }
}
