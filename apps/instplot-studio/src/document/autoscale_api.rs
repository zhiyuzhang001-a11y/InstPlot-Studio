use super::*;

impl FigureDocument {
    pub fn refresh_autoscale(&mut self) -> Result<(), String> {
        let mut candidate = self.project.clone();
        refresh_active_autoscales(&mut candidate)?;
        candidate.validate().map_err(|error| error.to_string())?;
        self.project = candidate;
        Ok(())
    }

    pub fn restore_autoscale_after_data_change(
        &mut self,
        identities: &[AxisIdentity],
    ) -> Result<(), String> {
        let mut candidate = self.project.clone();
        restore_autoscale_for_axes(&mut candidate, identities.iter().copied())?;
        candidate.validate().map_err(|error| error.to_string())?;
        self.project = candidate;
        Ok(())
    }
}
