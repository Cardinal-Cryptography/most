use scale::Output;

pub struct CallInput<'a>(pub &'a [u8]);

impl<'a> scale::Encode for CallInput<'a> {
    fn encode_to<T: Output + ?Sized>(&self, dest: &mut T) {
        dest.write(self.0);
    }
}

pub type Selector = [u8; 4];
